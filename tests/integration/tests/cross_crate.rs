use std::fs;
use std::path::Path;
use tempfile::TempDir;

use notcore::{HookPhase, HookSpec};
use notfiles::{LinkOptions, link};
use nothooks::{HookResult, HookRunner};
use notsecrets::config::{Provider, SecretsConfig};
use notsecrets::resolver::SecretResolver;
use notsecrets::sources::IdentitySource;
use notsecrets::{FileSource, resolve_identities};

/// Test that notsecrets and nothooks can each be used independently
/// in the same integration boundary — resolving an age key from a file
/// and running a dot-phase hook both succeed without coupling.
#[test]
fn test_nothooks_notsecrets_independent() {
    let dir = TempDir::new().unwrap();

    // Write a fake age key via FileSource
    let key_path = dir.path().join("age.key");
    fs::write(
        &key_path,
        "AGE-SECRET-KEY-1X3QKFQ4MZQM7LTJ3AX0N3EM63RGRV4J6N5ZDWPVKCEUCZKJWJSUSU6GYN6\n",
    )
    .unwrap();

    let sources: Vec<Box<dyn IdentitySource>> = vec![Box::new(FileSource::new(key_path))];
    let identities = resolve_identities(sources).expect("resolve_identities failed");
    assert!(!identities.is_empty(), "expected at least one identity");

    // Write a hook that just prints
    let script = dir.path().join("chain.nu");
    fs::write(&script, "print chain-ok\n").unwrap();

    let spec = HookSpec {
        name: "chain".to_string(),
        script: script.to_str().unwrap().to_string(),
        phase: HookPhase::Dot,
        interpreter: None,
    };

    let runner = HookRunner::new(dir.path().to_path_buf());
    let result = runner.run_hook(&spec).expect("hook runner should not fail");
    assert!(
        matches!(result, HookResult::Ok),
        "expected HookResult::Ok, got: {result:?}"
    );
}

#[test]
fn secret_resolver_resolves_env_var_cross_crate() {
    use std::collections::HashMap;

    unsafe { std::env::set_var("NOTFILES_CROSS_CRATE_TEST", "works") };

    let config = SecretsConfig {
        providers: vec![Provider::Env],
        provider: HashMap::new(),
        secrets: HashMap::new(),
    };
    let resolver = SecretResolver::from_config(config).unwrap();
    let val = resolver.resolve("NOTFILES_CROSS_CRATE_TEST").unwrap();
    assert_eq!(val, Some("works".to_string()));

    unsafe { std::env::remove_var("NOTFILES_CROSS_CRATE_TEST") };
}

// ── nu_libs nushell package e2e ───────────────────────────────────────────────

/// Helper: build a dotfiles dir containing the nushell/nu_libs.nu package
/// mirroring the real notfiles repo layout.
fn make_nu_libs_dotfiles(dotfiles: &Path, target_home: &Path) {
    let src = dotfiles.join("nushell/.config/nushell/autoload");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("nu_libs.nu"),
        b"const NU_LIBS_AI = \"/Users/joe/dev/nu_libs/lib/ai/mod.nu\"\nuse $NU_LIBS_AI *\n",
    )
    .unwrap();
    fs::write(
        dotfiles.join("notfiles.toml"),
        format!(
            "[defaults]\ntarget = \"{}\"\ninclude = [\"nushell\"]\n\n[packages.nushell]\n",
            target_home.display()
        )
        .as_bytes(),
    )
    .unwrap();
}

/// The nu_libs autoload file is symlinked into the correct autoload directory.
#[test]
fn nu_libs_autoload_is_linked_to_target() {
    let dotfiles = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let autoload = home.path().join(".config/nushell/autoload");
    fs::create_dir_all(&autoload).unwrap();

    make_nu_libs_dotfiles(dotfiles.path(), home.path());

    let opts = LinkOptions {
        force: false,
        no_backup: false,
        dry_run: false,
        verbose: false,
    };
    link(dotfiles.path(), &["nushell".to_string()], &opts).unwrap();

    let link_path = autoload.join("nu_libs.nu");
    assert!(link_path.exists(), "nu_libs.nu should exist at target");
    assert!(link_path.is_symlink(), "nu_libs.nu should be a symlink");
    assert_eq!(
        fs::read_link(&link_path).unwrap(),
        dotfiles
            .path()
            .join("nushell/.config/nushell/autoload/nu_libs.nu"),
        "symlink should point into the notfiles nushell package"
    );
}

/// Unlinking removes the symlink but leaves the autoload directory intact.
#[test]
fn nu_libs_autoload_is_removed_on_unlink() {
    let dotfiles = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let autoload = home.path().join(".config/nushell/autoload");
    fs::create_dir_all(&autoload).unwrap();

    make_nu_libs_dotfiles(dotfiles.path(), home.path());

    let opts = LinkOptions {
        force: false,
        no_backup: false,
        dry_run: false,
        verbose: false,
    };
    link(dotfiles.path(), &["nushell".to_string()], &opts).unwrap();
    notfiles::unlink(dotfiles.path(), &["nushell".to_string()], &opts).unwrap();

    assert!(
        !autoload.join("nu_libs.nu").exists(),
        "symlink should be removed after unlink"
    );
}

/// A pre-existing regular file at the target path causes link to fail without --force.
#[test]
fn nu_libs_link_does_not_overwrite_existing_file_without_force() {
    let dotfiles = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let autoload = home.path().join(".config/nushell/autoload");
    fs::create_dir_all(&autoload).unwrap();
    fs::write(autoload.join("nu_libs.nu"), b"# existing\n").unwrap();

    make_nu_libs_dotfiles(dotfiles.path(), home.path());

    let opts = LinkOptions {
        force: false,
        no_backup: false,
        dry_run: false,
        verbose: false,
    };
    let result = link(dotfiles.path(), &["nushell".to_string()], &opts);
    assert!(
        result.is_err(),
        "link should fail when target file exists without --force"
    );
}

/// With --force the existing file is replaced by a symlink.
#[test]
fn nu_libs_link_with_force_replaces_existing_file() {
    let dotfiles = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let autoload = home.path().join(".config/nushell/autoload");
    fs::create_dir_all(&autoload).unwrap();
    fs::write(autoload.join("nu_libs.nu"), b"# existing\n").unwrap();

    make_nu_libs_dotfiles(dotfiles.path(), home.path());

    let opts = LinkOptions {
        force: true,
        no_backup: false,
        dry_run: false,
        verbose: false,
    };
    link(dotfiles.path(), &["nushell".to_string()], &opts).unwrap();

    assert!(
        autoload.join("nu_libs.nu").is_symlink(),
        "nu_libs.nu should be a symlink after forced link"
    );
}

/// Test that notfiles ignores .notfiles-state.toml and .nothooks-state.toml
/// by default — they must not be symlinked into the target directory and must
/// not appear in the returned State.
#[test]
fn test_notfiles_respects_default_ignore() {
    let dotfiles = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let d = dotfiles.path();

    // notfiles.toml — package "pkg" targeting home tempdir
    fs::write(
        d.join("notfiles.toml"),
        format!(
            "[defaults]\nmethod = \"symlink\"\ntarget = \"{}\"\n",
            home.path().display()
        ),
    )
    .unwrap();

    // Package dir with a normal file and two state files that should be ignored
    let pkg = d.join("pkg");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(pkg.join("foo.txt"), "hello\n").unwrap();
    fs::write(pkg.join(".notfiles-state.toml"), "# state\n").unwrap();
    fs::write(pkg.join(".nothooks-state.toml"), "# state\n").unwrap();

    let opts = LinkOptions {
        force: false,
        no_backup: false,
        dry_run: false,
        verbose: false,
    };
    let state = link(d, &[], &opts).expect("link() failed");

    // foo.txt linked
    assert!(
        home.path().join("foo.txt").exists(),
        "foo.txt should be linked"
    );

    // state files NOT linked
    assert!(
        !home.path().join(".notfiles-state.toml").exists(),
        ".notfiles-state.toml must not be linked"
    );
    assert!(
        !home.path().join(".nothooks-state.toml").exists(),
        ".nothooks-state.toml must not be linked"
    );

    // state doesn't record them either
    let names: Vec<_> = state
        .entries
        .iter()
        .map(|e| {
            Path::new(&e.source)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert!(
        !names.iter().any(|n| n == ".notfiles-state.toml"),
        ".notfiles-state.toml must not be in state entries"
    );
    assert!(
        !names.iter().any(|n| n == ".nothooks-state.toml"),
        ".nothooks-state.toml must not be in state entries"
    );
}
