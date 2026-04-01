use std::fs;
use std::path::Path;
use tempfile::TempDir;

use notcore::{HookPhase, HookSpec};
use nothooks::{HookResult, HookRunner};
use notsecrets::{resolve_age_key, AgeKeySource, FileSource};
use notfiles::{link, LinkOptions};

/// Test that notsecrets and nothooks can each be used independently
/// in the same integration boundary — resolving an age key from a file
/// and running a dot-phase hook both succeed without coupling.
#[test]
fn test_nothooks_notsecrets_independent() {
    let dir = TempDir::new().unwrap();

    // Write a fake age key via FileSource
    let key_path = dir.path().join("age.key");
    fs::write(&key_path, "AGE-SECRET-KEY-1CROSSCRATE\n").unwrap();

    let sources: Vec<Box<dyn AgeKeySource>> = vec![Box::new(FileSource::new(key_path))];
    let key = resolve_age_key(sources).unwrap();
    assert!(
        key.trim().starts_with("AGE-SECRET-KEY-"),
        "expected age key prefix, got: {key:?}"
    );

    // Write a hook that just prints
    let script = dir.path().join("chain.nu");
    fs::write(&script, "print chain-ok\n").unwrap();

    let spec = HookSpec {
        name: "chain".to_string(),
        script: script.to_str().unwrap().to_string(),
        phase: HookPhase::Dot,
    };

    let runner = HookRunner::new(dir.path().to_path_buf());
    let result = runner.run_hook(&spec);
    assert!(
        matches!(result, HookResult::Ok),
        "expected HookResult::Ok, got: {result:?}"
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

    let opts = LinkOptions { force: false, no_backup: false, dry_run: false, verbose: false };
    let state = link(d, &[], &opts).unwrap();

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
        !names.contains(&".notfiles-state.toml".to_string()),
        ".notfiles-state.toml must not be in state entries"
    );
    assert!(
        !names.contains(&".nothooks-state.toml".to_string()),
        ".nothooks-state.toml must not be in state entries"
    );
}
