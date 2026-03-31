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
    let output = Command::new(notfiles_bin())
        .arg("--dir")
        .arg(dotfiles)
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
    assert!(stdout.contains("Created notfiles.toml"));
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
    assert!(gitconfig.symlink_metadata().unwrap().file_type().is_symlink());

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
    assert!(target.join(".gitconfig").symlink_metadata().unwrap().file_type().is_symlink());

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
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .contains("notfiles-backup")
        })
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
    assert!(!ssh_config.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(&ssh_config).unwrap(), "Host *\n  AddKeysToAgent yes");

    // Status should show "copied"
    let (stdout, _, _) = run(&dotfiles, &["status", "ssh"]);
    assert!(stdout.contains("copied"));

    // Unlink should remove the copy
    let (_, _, ok) = run(&dotfiles, &["unlink", "ssh"]);
    assert!(ok);
    assert!(!ssh_config.exists());
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
