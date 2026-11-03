use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn notfiles_bin() -> std::path::PathBuf {
    // Built by `cargo test`
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // remove test binary name
    path.pop(); // remove `deps`
    path.push("notfiles");
    path
}

fn run(dotfiles: &Path, args: &[&str]) -> (String, String, bool) {
    let config = dotfiles.join("notfiles.toml");
    let output = Command::new(notfiles_bin())
        .arg("--dir")
        .arg(dotfiles)
        .arg("--config")
        .arg(&config)
        .args(args)
        .output()
        .expect("failed to run notfiles");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

fn setup_dotfiles(tmp: &TempDir) {
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");
    fs::create_dir_all(&dotfiles).unwrap();
    fs::create_dir_all(&target).unwrap();

    // Create a zsh package
    let zsh = dotfiles.join("zsh");
    fs::create_dir_all(zsh.join(".config/zsh")).unwrap();
    fs::write(zsh.join(".config/zsh/zshrc"), "# zshrc content").unwrap();
    fs::write(zsh.join(".zshenv"), "# zshenv content").unwrap();

    // Create a git package
    let git = dotfiles.join("git");
    fs::create_dir_all(&git).unwrap();
    fs::write(git.join(".gitconfig"), "[user]\nname = Test").unwrap();

    // Write config pointing target to our temp home
    let config = format!(
        r#"[defaults]
target = "{}"
ignore = [".git", ".DS_Store", "README.md", "LICENSE", "notfiles.toml", ".notfiles-state.toml"]
"#,
        target.display()
    );
    fs::write(dotfiles.join("notfiles.toml"), config).unwrap();
}

#[test]
fn test_init_creates_config() {
    let tmp = TempDir::new().unwrap();
    let dotfiles = tmp.path().join("dotfiles");
    fs::create_dir_all(&dotfiles).unwrap();

    let (stdout, _, ok) = run(&dotfiles, &["init"]);
    assert!(ok);
    assert!(stdout.contains("Created"), "expected Created in: {stdout}");
    assert!(dotfiles.join("notfiles.toml").exists());
}

#[test]
fn test_init_idempotent() {
    let tmp = TempDir::new().unwrap();
    let dotfiles = tmp.path().join("dotfiles");
    fs::create_dir_all(&dotfiles).unwrap();

    run(&dotfiles, &["init"]);
    let (stdout, _, ok) = run(&dotfiles, &["init"]);
    assert!(ok);
    assert!(stdout.contains("already exists"));
}

#[test]
fn test_link_and_status() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    let (stdout, stderr, ok) = run(&dotfiles, &["link", "--verbose"]);
    assert!(ok, "link failed: stdout={stdout} stderr={stderr}");

    // Verify symlinks exist
    let zshrc = target.join(".config/zsh/zshrc");
    assert!(zshrc.exists(), "zshrc should exist");
    assert!(zshrc.symlink_metadata().unwrap().file_type().is_symlink());

    let gitconfig = target.join(".gitconfig");
    assert!(gitconfig.exists());
    assert!(
        gitconfig
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );

    // State file should exist
    assert!(dotfiles.join(".notfiles-state.toml").exists());

    // Status should show linked
    let (stdout, _, ok) = run(&dotfiles, &["status"]);
    assert!(ok);
    assert!(stdout.contains("linked"));
}

#[test]
fn test_link_idempotent() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");

    let (_, _, ok) = run(&dotfiles, &["link"]);
    assert!(ok);

    // Link again — should succeed (skips already linked)
    let (_, _, ok) = run(&dotfiles, &["link"]);
    assert!(ok);
}

#[test]
fn test_link_specific_package() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    let (_, _, ok) = run(&dotfiles, &["link", "zsh"]);
    assert!(ok);

    assert!(target.join(".config/zsh/zshrc").exists());
    assert!(!target.join(".gitconfig").exists());
}

#[test]
fn test_unlink() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    run(&dotfiles, &["link"]);
    assert!(target.join(".gitconfig").exists());

    let (_, _, ok) = run(&dotfiles, &["unlink", "--verbose"]);
    assert!(ok);

    assert!(!target.join(".gitconfig").exists());
    assert!(!target.join(".config/zsh/zshrc").exists());
}

#[test]
fn test_unlink_specific_package() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    run(&dotfiles, &["link"]);

    let (_, _, ok) = run(&dotfiles, &["unlink", "git"]);
    assert!(ok);

    assert!(!target.join(".gitconfig").exists());
    // zsh should still be linked
    assert!(target.join(".config/zsh/zshrc").exists());
}

#[test]
fn test_conflict_without_force() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    // Pre-create a conflicting file
    fs::write(target.join(".gitconfig"), "existing content").unwrap();

    let (_, stderr, ok) = run(&dotfiles, &["link"]);
    assert!(!ok, "should fail on conflict");
    assert!(
        stderr.contains("conflict") || stderr.contains("already exists"),
        "stderr: {stderr}"
    );
}

#[test]
fn test_force_with_backup() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    // Pre-create conflicting file
    fs::write(target.join(".gitconfig"), "old content").unwrap();

    let (_, _, ok) = run(&dotfiles, &["link", "--force", "--verbose"]);
    assert!(ok);

    // The link should now exist
    assert!(
        target
            .join(".gitconfig")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );

    // A backup should exist
    let backups: Vec<_> = fs::read_dir(&target)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".gitconfig.notfiles-backup-")
        })
        .collect();
    assert_eq!(backups.len(), 1);
    let backup_content = fs::read_to_string(backups[0].path()).unwrap();
    assert_eq!(backup_content, "old content");
}

#[test]
fn test_force_no_backup() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    fs::write(target.join(".gitconfig"), "old content").unwrap();

    let (_, _, ok) = run(&dotfiles, &["link", "--force", "--no-backup"]);
    assert!(ok);

    // No backup should exist
    let backups: Vec<_> = fs::read_dir(&target)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains("notfiles-backup"))
        .collect();
    assert_eq!(backups.len(), 0);
}

#[test]
fn test_copy_method() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    // Create an ssh package with copy method
    let ssh = dotfiles.join("ssh");
    fs::create_dir_all(ssh.join(".ssh")).unwrap();
    fs::write(ssh.join(".ssh/config"), "Host *\n  AddKeysToAgent yes").unwrap();

    // Update config to use copy for ssh
    let config = format!(
        r#"[defaults]
target = "{}"
ignore = [".git", ".DS_Store", "README.md", "LICENSE", "notfiles.toml", ".notfiles-state.toml"]

[packages.ssh]
method = "copy"
"#,
        target.display()
    );
    fs::write(dotfiles.join("notfiles.toml"), config).unwrap();

    let (_, _, ok) = run(&dotfiles, &["link", "ssh", "--verbose"]);
    assert!(ok);

    let ssh_config = target.join(".ssh/config");
    assert!(ssh_config.exists());
    // Should NOT be a symlink
    assert!(
        !ssh_config
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_to_string(&ssh_config).unwrap(),
        "Host *\n  AddKeysToAgent yes"
    );

    // Status should show "copied"
    let (stdout, _, _) = run(&dotfiles, &["status", "ssh"]);
    assert!(stdout.contains("copied"));

    // Unlink should remove the copy
    let (_, _, ok) = run(&dotfiles, &["unlink", "ssh"]);
    assert!(ok);
    assert!(!ssh_config.exists());
}

#[test]
fn test_status_flags_diverged_copy_as_conflict() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    let ssh = dotfiles.join("ssh");
    fs::create_dir_all(ssh.join(".ssh")).unwrap();
    fs::write(ssh.join(".ssh/config"), "Host *\n  AddKeysToAgent yes").unwrap();

    let config = format!(
        r#"[defaults]
target = "{}"
ignore = [".git", ".DS_Store", "README.md", "LICENSE", "notfiles.toml", ".notfiles-state.toml"]

[packages.ssh]
method = "copy"
"#,
        target.display()
    );
    fs::write(dotfiles.join("notfiles.toml"), config).unwrap();

    let (_, _, ok) = run(&dotfiles, &["link", "ssh"]);
    assert!(ok);

    let ssh_config = target.join(".ssh/config");
    fs::write(&ssh_config, "Host github.com\n  User joe\n").unwrap();

    let (stdout, _, ok) = run(&dotfiles, &["status", "ssh"]);
    assert!(ok);
    assert!(
        stdout.contains("conflict"),
        "diverged copy should be reported as conflict, stdout={stdout}"
    );
}

#[test]
fn test_unlink_preserves_diverged_copy_and_state() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    let ssh = dotfiles.join("ssh");
    fs::create_dir_all(ssh.join(".ssh")).unwrap();
    fs::write(ssh.join(".ssh/config"), "Host *\n  AddKeysToAgent yes").unwrap();

    let config = format!(
        r#"[defaults]
target = "{}"
ignore = [".git", ".DS_Store", "README.md", "LICENSE", "notfiles.toml", ".notfiles-state.toml"]

[packages.ssh]
method = "copy"
"#,
        target.display()
    );
    fs::write(dotfiles.join("notfiles.toml"), config).unwrap();

    let (_, _, ok) = run(&dotfiles, &["link", "ssh"]);
    assert!(ok);

    let ssh_config = target.join(".ssh/config");
    fs::write(&ssh_config, "Host github.com\n  User joe\n").unwrap();

    let (stdout, stderr, ok) = run(&dotfiles, &["unlink", "ssh", "--verbose"]);
    assert!(
        ok,
        "unlink should not fail: stdout={stdout} stderr={stderr}"
    );
    assert!(
        ssh_config.exists(),
        "diverged copy should not be removed during unlink"
    );

    let state = fs::read_to_string(dotfiles.join(".notfiles-state.toml")).unwrap();
    assert!(
        state.contains(".ssh/config"),
        "state entry must be retained when unlink skips a diverged copy: {state}"
    );
}

#[test]
fn test_dry_run() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    let (stdout, _, ok) = run(&dotfiles, &["--dry-run", "link"]);
    assert!(ok);
    assert!(stdout.contains("dry run"));
    assert!(stdout.contains("would link"));

    // Nothing should actually be created
    assert!(!target.join(".gitconfig").exists());
    assert!(!dotfiles.join(".notfiles-state.toml").exists());
}

#[test]
fn test_status_missing() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");

    // Don't link anything — status should show missing
    let (stdout, _, ok) = run(&dotfiles, &["status"]);
    assert!(ok);
    assert!(stdout.contains("missing"));
}

#[test]
fn test_ignore_patterns() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    // Add a README to a package — it should be ignored
    fs::write(dotfiles.join("zsh/README.md"), "read me").unwrap();
    fs::write(dotfiles.join("zsh/.DS_Store"), "junk").unwrap();

    let (_, _, ok) = run(&dotfiles, &["link"]);
    assert!(ok);

    assert!(!target.join("README.md").exists());
    assert!(!target.join(".DS_Store").exists());
    // But real files should be linked
    assert!(target.join(".config/zsh/zshrc").exists());
}

#[test]
fn test_package_not_found() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");

    let (_, stderr, ok) = run(&dotfiles, &["link", "nonexistent"]);
    assert!(!ok);
    assert!(stderr.contains("nonexistent"), "stderr: {stderr}");
}

#[test]
fn test_unlink_cleans_empty_dirs() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    let (_, _, ok) = run(&dotfiles, &["link", "zsh"]);
    assert!(ok);
    assert!(target.join(".config/zsh").is_dir());

    let (_, _, ok) = run(&dotfiles, &["unlink", "zsh"]);
    assert!(ok);

    // The .config/zsh directory should be cleaned up
    assert!(!target.join(".config/zsh").exists());
}

// ── Fixture-based tests ─────────────────────────────────────────────────────

/// Copy the static fixture into a temp dir with a rewritten target, so tests
/// don't touch the real home directory.
fn setup_from_fixture(tmp: &TempDir) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dotfiles");
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");
    fs::create_dir_all(&target).unwrap();
    copy_dir_recursive(&fixture, &dotfiles).unwrap();

    // Rewrite notfiles.toml to point target at our temp home
    let config_path = dotfiles.join("notfiles.toml");
    let content = fs::read_to_string(&config_path).unwrap();
    let rewritten = content.replace(
        "target = \"~\"",
        &format!("target = \"{}\"", target.display()),
    );
    fs::write(&config_path, rewritten).unwrap();
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[test]
fn test_fixture_include_excludes_non_packages() {
    let tmp = TempDir::new().unwrap();
    setup_from_fixture(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    let (_, _, ok) = run(&dotfiles, &["link"]);
    assert!(ok);

    // Stow packages should be linked
    assert!(
        target.join(".gitconfig").exists(),
        ".gitconfig should be linked"
    );
    assert!(target.join(".zshrc").exists(), ".zshrc should be linked");
    assert!(
        target.join(".config/nushell/config.nu").exists(),
        "nushell config should be linked",
    );
    assert!(
        target.join(".config/starship.toml").exists(),
        "starship config should be linked",
    );

    // Non-package dirs should NOT be linked
    assert!(
        !target.join("lib/common.sh").exists(),
        "scripts/ should not be linked",
    );
    assert!(
        !target.join("bootstrap-runbook.md").exists(),
        "docs/ should not be linked",
    );
    assert!(
        !target.join("doctor.sh").exists(),
        "scripts/ should not be linked",
    );
}

#[test]
fn test_fixture_status_shows_all_packages() {
    let tmp = TempDir::new().unwrap();
    setup_from_fixture(&tmp);
    let dotfiles = tmp.path().join("dotfiles");

    let (stdout, _, ok) = run(&dotfiles, &["status"]);
    assert!(ok);

    // Should list stow packages
    assert!(stdout.contains("git"), "status should show git package");
    assert!(stdout.contains("zsh"), "status should show zsh package");
    assert!(
        stdout.contains("nushell"),
        "status should show nushell package"
    );

    // Should NOT list excluded dirs
    assert!(
        !stdout.contains("scripts"),
        "status should not show scripts"
    );
    assert!(!stdout.contains("docs"), "status should not show docs");
}

#[test]
fn test_fixture_deep_paths_link_correctly() {
    let tmp = TempDir::new().unwrap();
    setup_from_fixture(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    let (_, _, ok) = run(&dotfiles, &["link", "vscode"]);
    assert!(ok);

    let vscode_settings = target.join("Library/Application Support/Code/User/settings.json");
    assert!(
        vscode_settings.exists(),
        "deep path should be linked: {}",
        vscode_settings.display(),
    );
}

#[test]
fn test_fixture_platform_filter_skips_linux_on_macos() {
    if !cfg!(target_os = "macos") {
        return; // This test is macOS-specific
    }
    let tmp = TempDir::new().unwrap();
    setup_from_fixture(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    let (_, _, ok) = run(&dotfiles, &["link"]);
    assert!(ok);

    // nixos has platforms = ["linux"], so it should NOT be linked on macOS
    assert!(
        !target.join("configuration.nix").exists(),
        "nixos package should be skipped on macOS",
    );

    // But macos-compatible packages should be linked
    assert!(target.join(".gitconfig").exists());
}

#[test]
fn test_fixture_link_and_unlink_round_trip() {
    let tmp = TempDir::new().unwrap();
    setup_from_fixture(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    // Link all
    let (_, _, ok) = run(&dotfiles, &["link"]);
    assert!(ok);
    assert!(target.join(".gitconfig").exists());

    // Unlink all
    let (_, _, ok) = run(&dotfiles, &["unlink"]);
    assert!(ok);
    assert!(!target.join(".gitconfig").exists());
    assert!(!target.join(".zshrc").exists());
}
