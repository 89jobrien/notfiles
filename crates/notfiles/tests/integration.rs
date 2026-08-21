use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn notfiles_bin() -> std::path::PathBuf {
    // Set by cargo for integration tests; does not depend on the target directory layout.
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_notfiles"))
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
    fs::create_dir_all(&dotfiles).expect("create_dir_all should succeed");
    fs::create_dir_all(&target).expect("create_dir_all should succeed");

    // Create a zsh package
    let zsh = dotfiles.join("zsh");
    fs::create_dir_all(zsh.join(".config/zsh")).expect("create_dir_all should succeed");
    fs::write(zsh.join(".config/zsh/zshrc"), "# zshrc content").expect("write should succeed");
    fs::write(zsh.join(".zshenv"), "# zshenv content").expect("write should succeed");

    // Create a git package
    let git = dotfiles.join("git");
    fs::create_dir_all(&git).expect("create_dir_all should succeed");
    fs::write(git.join(".gitconfig"), "[user]\nname = Test").expect("write should succeed");

    // Write config pointing target to our temp home
    let config = format!(
        r#"[defaults]
target = "{}"
ignore = [".git", ".DS_Store", "README.md", "LICENSE", "notfiles.toml", ".notfiles-state.toml"]
"#,
        target.display()
    );
    fs::write(dotfiles.join("notfiles.toml"), config).expect("write should succeed");
}

#[test]
fn test_init_creates_config() {
    let tmp = TempDir::new().expect("new should succeed");
    let dotfiles = tmp.path().join("dotfiles");
    fs::create_dir_all(&dotfiles).expect("create_dir_all should succeed");

    let (stdout, _, ok) = run(&dotfiles, &["init"]);
    assert!(ok);
    assert!(stdout.contains("Created notfiles.toml"));
    assert!(dotfiles.join("notfiles.toml").exists());
}

#[test]
fn test_init_idempotent() {
    let tmp = TempDir::new().expect("new should succeed");
    let dotfiles = tmp.path().join("dotfiles");
    fs::create_dir_all(&dotfiles).expect("create_dir_all should succeed");

    run(&dotfiles, &["init"]);
    let (stdout, _, ok) = run(&dotfiles, &["init"]);
    assert!(ok);
    assert!(stdout.contains("already exists"));
}

#[test]
fn test_link_and_status() {
    let tmp = TempDir::new().expect("new should succeed");
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    let (stdout, stderr, ok) = run(&dotfiles, &["link", "--verbose"]);
    assert!(ok, "link failed: stdout={stdout} stderr={stderr}");

    // Verify symlinks exist
    let zshrc = target.join(".config/zsh/zshrc");
    assert!(zshrc.exists(), "zshrc should exist");
    assert!(
        zshrc
            .symlink_metadata()
            .expect("symlink_metadata should succeed")
            .file_type()
            .is_symlink()
    );

    let gitconfig = target.join(".gitconfig");
    assert!(gitconfig.exists());
    assert!(
        gitconfig
            .symlink_metadata()
            .expect("symlink_metadata should succeed")
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
    let tmp = TempDir::new().expect("new should succeed");
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
    let tmp = TempDir::new().expect("new should succeed");
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
    let tmp = TempDir::new().expect("new should succeed");
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
    let tmp = TempDir::new().expect("new should succeed");
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
    let tmp = TempDir::new().expect("new should succeed");
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    // Pre-create a conflicting file
    fs::write(target.join(".gitconfig"), "existing content").expect("write should succeed");

    let (_, stderr, ok) = run(&dotfiles, &["link"]);
    assert!(!ok, "should fail on conflict");
    assert!(
        stderr.contains("conflict") || stderr.contains("already exists"),
        "stderr: {stderr}"
    );
}

#[test]
fn test_force_with_backup() {
    let tmp = TempDir::new().expect("new should succeed");
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    // Pre-create conflicting file
    fs::write(target.join(".gitconfig"), "old content").expect("write should succeed");

    let (_, _, ok) = run(&dotfiles, &["link", "--force", "--verbose"]);
    assert!(ok);

    // The link should now exist
    assert!(
        target
            .join(".gitconfig")
            .symlink_metadata()
            .expect("symlink_metadata should succeed")
            .file_type()
            .is_symlink()
    );

    // A backup should exist
    let backups: Vec<_> = fs::read_dir(&target)
        .expect("read_dir should succeed")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".gitconfig.notfiles-backup-")
        })
        .collect();
    assert_eq!(backups.len(), 1);
    let backup_content =
        fs::read_to_string(backups[0].path()).expect("read_to_string should succeed");
    assert_eq!(backup_content, "old content");
}

#[test]
fn test_force_no_backup() {
    let tmp = TempDir::new().expect("new should succeed");
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    fs::write(target.join(".gitconfig"), "old content").expect("write should succeed");

    let (_, _, ok) = run(&dotfiles, &["link", "--force", "--no-backup"]);
    assert!(ok);

    // No backup should exist
    let backups: Vec<_> = fs::read_dir(&target)
        .expect("read_dir should succeed")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains("notfiles-backup"))
        .collect();
    assert_eq!(backups.len(), 0);
}

#[test]
fn test_copy_method() {
    let tmp = TempDir::new().expect("new should succeed");
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    // Create an ssh package with copy method
    let ssh = dotfiles.join("ssh");
    fs::create_dir_all(ssh.join(".ssh")).expect("create_dir_all should succeed");
    fs::write(ssh.join(".ssh/config"), "Host *\n  AddKeysToAgent yes")
        .expect("write should succeed");

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
    fs::write(dotfiles.join("notfiles.toml"), config).expect("write should succeed");

    let (_, _, ok) = run(&dotfiles, &["link", "ssh", "--verbose"]);
    assert!(ok);

    let ssh_config = target.join(".ssh/config");
    assert!(ssh_config.exists());
    // Should NOT be a symlink
    assert!(
        !ssh_config
            .symlink_metadata()
            .expect("symlink_metadata should succeed")
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_to_string(&ssh_config).expect("read_to_string should succeed"),
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
    let tmp = TempDir::new().expect("new should succeed");
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    let ssh = dotfiles.join("ssh");
    fs::create_dir_all(ssh.join(".ssh")).expect("create_dir_all should succeed");
    fs::write(ssh.join(".ssh/config"), "Host *\n  AddKeysToAgent yes")
        .expect("write should succeed");

    let config = format!(
        r#"[defaults]
target = "{}"
ignore = [".git", ".DS_Store", "README.md", "LICENSE", "notfiles.toml", ".notfiles-state.toml"]

[packages.ssh]
method = "copy"
"#,
        target.display()
    );
    fs::write(dotfiles.join("notfiles.toml"), config).expect("write should succeed");

    let (_, _, ok) = run(&dotfiles, &["link", "ssh"]);
    assert!(ok);

    let ssh_config = target.join(".ssh/config");
    fs::write(&ssh_config, "Host github.com\n  User joe\n").expect("write should succeed");

    let (stdout, _, ok) = run(&dotfiles, &["status", "ssh"]);
    assert!(ok);
    assert!(
        stdout.contains("conflict"),
        "diverged copy should be reported as conflict, stdout={stdout}"
    );
}

#[test]
fn test_unlink_preserves_diverged_copy_and_state() {
    let tmp = TempDir::new().expect("new should succeed");
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    let ssh = dotfiles.join("ssh");
    fs::create_dir_all(ssh.join(".ssh")).expect("create_dir_all should succeed");
    fs::write(ssh.join(".ssh/config"), "Host *\n  AddKeysToAgent yes")
        .expect("write should succeed");

    let config = format!(
        r#"[defaults]
target = "{}"
ignore = [".git", ".DS_Store", "README.md", "LICENSE", "notfiles.toml", ".notfiles-state.toml"]

[packages.ssh]
method = "copy"
"#,
        target.display()
    );
    fs::write(dotfiles.join("notfiles.toml"), config).expect("write should succeed");

    let (_, _, ok) = run(&dotfiles, &["link", "ssh"]);
    assert!(ok);

    let ssh_config = target.join(".ssh/config");
    fs::write(&ssh_config, "Host github.com\n  User joe\n").expect("write should succeed");

    let (stdout, stderr, ok) = run(&dotfiles, &["unlink", "ssh", "--verbose"]);
    assert!(
        ok,
        "unlink should not fail: stdout={stdout} stderr={stderr}"
    );
    assert!(
        ssh_config.exists(),
        "diverged copy should not be removed during unlink"
    );

    let state = fs::read_to_string(dotfiles.join(".notfiles-state.toml"))
        .expect("read_to_string should succeed");
    assert!(
        state.contains(".ssh/config"),
        "state entry must be retained when unlink skips a diverged copy: {state}"
    );
}

#[test]
fn test_dry_run() {
    let tmp = TempDir::new().expect("new should succeed");
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
    let tmp = TempDir::new().expect("new should succeed");
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");

    // Don't link anything — status should show missing
    let (stdout, _, ok) = run(&dotfiles, &["status"]);
    assert!(ok);
    assert!(stdout.contains("missing"));
}

#[test]
fn test_ignore_patterns() {
    let tmp = TempDir::new().expect("new should succeed");
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    // Add a README to a package — it should be ignored
    fs::write(dotfiles.join("zsh/README.md"), "read me").expect("write should succeed");
    fs::write(dotfiles.join("zsh/.DS_Store"), "junk").expect("write should succeed");

    let (_, _, ok) = run(&dotfiles, &["link"]);
    assert!(ok);

    assert!(!target.join("README.md").exists());
    assert!(!target.join(".DS_Store").exists());
    // But real files should be linked
    assert!(target.join(".config/zsh/zshrc").exists());
}

#[test]
fn test_package_not_found() {
    let tmp = TempDir::new().expect("new should succeed");
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");

    let (_, stderr, ok) = run(&dotfiles, &["link", "nonexistent"]);
    assert!(!ok);
    assert!(stderr.contains("nonexistent"), "stderr: {stderr}");
}

#[test]
fn test_unlink_cleans_empty_dirs() {
    let tmp = TempDir::new().expect("new should succeed");
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
    fs::create_dir_all(&target).expect("create_dir_all should succeed");
    copy_dir_recursive(&fixture, &dotfiles).expect("copy_dir_recursive should succeed");

    // Rewrite notfiles.toml to point target at our temp home
    let config_path = dotfiles.join("notfiles.toml");
    let content = fs::read_to_string(&config_path).expect("read_to_string should succeed");
    let rewritten = content.replace(
        "target = \"~\"",
        &format!("target = \"{}\"", target.display()),
    );
    fs::write(&config_path, rewritten).expect("write should succeed");
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
    let tmp = TempDir::new().expect("new should succeed");
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
    let tmp = TempDir::new().expect("new should succeed");
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
    let tmp = TempDir::new().expect("new should succeed");
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
    let tmp = TempDir::new().expect("new should succeed");
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
    let tmp = TempDir::new().expect("new should succeed");
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

#[cfg(unix)]
#[test]
fn test_copy_preserves_executable_bit() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().expect("new should succeed");
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");
    fs::create_dir_all(&target).expect("create_dir_all should succeed");

    let bin = dotfiles.join("bin");
    fs::create_dir_all(bin.join(".local/bin")).expect("create_dir_all should succeed");
    let script = bin.join(".local/bin/tool");
    fs::write(&script, "#!/bin/sh\necho hi\n").expect("write should succeed");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
        .expect("set_permissions should succeed");

    let config = format!(
        r#"[defaults]
target = "{}"
ignore = [".git", ".DS_Store", "notfiles.toml", ".notfiles-state.toml"]

[packages.bin]
method = "copy"
"#,
        target.display()
    );
    fs::write(dotfiles.join("notfiles.toml"), config).expect("write should succeed");

    let (_, _, ok) = run(&dotfiles, &["link", "bin"]);
    assert!(ok);

    let copied = target.join(".local/bin/tool");
    let mode = copied
        .metadata()
        .expect("metadata should succeed")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o755, "copied script lost its executable bit");
}

fn write_mixed_repo(
    tmp: &TempDir,
    packages_line: &str,
) -> (std::path::PathBuf, std::path::PathBuf) {
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");
    fs::create_dir_all(&target).expect("create_dir_all should succeed");
    fs::create_dir_all(dotfiles.join("git")).expect("create_dir_all should succeed");
    fs::write(dotfiles.join("git/.gitconfig"), "[user]\nname = T").expect("write should succeed");
    fs::create_dir_all(dotfiles.join("scripts")).expect("create_dir_all should succeed");
    fs::write(dotfiles.join("scripts/deploy.sh"), "#!/bin/sh").expect("write should succeed");
    fs::create_dir_all(dotfiles.join("docs")).expect("create_dir_all should succeed");
    fs::write(dotfiles.join("docs/guide.md"), "# guide").expect("write should succeed");
    fs::write(
        dotfiles.join("notfiles.toml"),
        format!(
            "[defaults]\ntarget = \"{}\"\n{packages_line}\n",
            target.display()
        ),
    )
    .expect("write should succeed");
    (dotfiles, target)
}

#[test]
fn test_declared_packages_do_not_link_non_package_dirs() {
    let tmp = TempDir::new().expect("new should succeed");
    let (dotfiles, target) = write_mixed_repo(&tmp, "packages = [\"git\"]");

    let (_, stderr, ok) = run(&dotfiles, &["link"]);
    assert!(ok, "{stderr}");
    assert!(target.join(".gitconfig").exists());
    assert!(!target.join("deploy.sh").exists(), "scripts/ must not link");
    assert!(!target.join("guide.md").exists(), "docs/ must not link");
    assert!(
        !stderr.contains("deprecated"),
        "no deprecation warning when packages is declared: {stderr}"
    );

    let (_, _, ok) = run(&dotfiles, &["link", "scripts"]);
    assert!(!ok, "requesting a non-declared directory should fail");
}

#[test]
fn test_missing_declared_package_is_an_error_with_suggestion() {
    let tmp = TempDir::new().expect("new should succeed");
    let (dotfiles, _) = write_mixed_repo(&tmp, "packages = [\"gti\"]");

    let (_, stderr, ok) = run(&dotfiles, &["link"]);
    assert!(!ok);
    assert!(stderr.contains("did you mean 'git'"), "{stderr}");
}

#[test]
fn test_undeclared_packages_warn_about_deprecated_auto_discovery() {
    let tmp = TempDir::new().expect("new should succeed");
    let (dotfiles, target) = write_mixed_repo(&tmp, "");

    let (_, stderr, ok) = run(&dotfiles, &["link"]);
    assert!(ok, "{stderr}");
    assert!(stderr.contains("deprecated"), "{stderr}");
    assert!(stderr.contains("packages"), "{stderr}");
    // Legacy behavior retained for now.
    assert!(target.join("deploy.sh").exists());
}

#[test]
fn test_conflict_error_message_and_target_untouched() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    fs::write(target.join(".gitconfig"), "existing content").unwrap();

    let (_, stderr, ok) = run(&dotfiles, &["link", "git"]);
    assert!(!ok);
    assert!(
        stderr.contains("already exists (use --force to overwrite)"),
        "stderr: {stderr}"
    );
    let meta = target.join(".gitconfig").symlink_metadata().unwrap();
    assert!(!meta.file_type().is_symlink());
    assert_eq!(
        fs::read_to_string(target.join(".gitconfig")).unwrap(),
        "existing content"
    );
}

#[test]
fn test_dry_run_force_leaves_conflicting_file_and_makes_no_backup() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    fs::write(target.join(".gitconfig"), "old content").unwrap();

    let (_, _, ok) = run(&dotfiles, &["--dry-run", "link", "--force"]);
    assert!(ok);

    let meta = target.join(".gitconfig").symlink_metadata().unwrap();
    assert!(!meta.file_type().is_symlink());
    assert_eq!(
        fs::read_to_string(target.join(".gitconfig")).unwrap(),
        "old content"
    );
    let names: Vec<String> = fs::read_dir(&target)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, vec![".gitconfig".to_string()], "home: {names:?}");
    assert!(!dotfiles.join(".notfiles-state.toml").exists());
}

#[test]
fn test_link_named_package_records_only_that_package_in_state() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");

    let (_, _, ok) = run(&dotfiles, &["link", "git"]);
    assert!(ok);

    let state = fs::read_to_string(dotfiles.join(".notfiles-state.toml")).unwrap();
    assert!(state.contains("package = \"git\""), "state: {state}");
    assert!(!state.contains("package = \"zsh\""), "state: {state}");
}

#[test]
fn test_check_fails_when_dotfiles_dir_missing() {
    let tmp = TempDir::new().unwrap();
    let missing = tmp.path().join("no-such-dotfiles");

    let (_, stderr, ok) = run(&missing, &["check"]);
    assert!(!ok, "check should fail for a missing directory");
    assert!(!stderr.trim().is_empty(), "expected an error on stderr");
}
#[test]
fn test_which_resolves_linked_path_to_package() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    let (_, _, ok) = run(&dotfiles, &["link"]);
    assert!(ok);

    let gitconfig = target.join(".gitconfig");
    let (stdout, _, ok) = run(&dotfiles, &["which", gitconfig.to_str().unwrap()]);
    assert!(ok);
    assert!(stdout.contains("package: git"), "{stdout}");
    assert!(
        stdout.contains(dotfiles.join("git/.gitconfig").to_str().unwrap()),
        "{stdout}",
    );
    assert!(stdout.contains("status:"), "{stdout}");
}

#[test]
fn test_which_json_reports_source_and_method() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    let (_, _, ok) = run(&dotfiles, &["link"]);
    assert!(ok);

    let zshrc = target.join(".config/zsh/zshrc");
    let (stdout, _, ok) = run(&dotfiles, &["--json", "which", zshrc.to_str().unwrap()]);
    assert!(ok);

    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let matches = parsed["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["package"], "zsh");
    assert_eq!(matches[0]["method"], "symlink");
    assert_eq!(matches[0]["via"], "state");
    assert_eq!(matches[0]["current"], true);
}

#[test]
fn test_which_reports_unlinked_file_from_config() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let target = tmp.path().join("home");

    // No `link` run — the file is configured but not yet placed.
    let gitconfig = target.join(".gitconfig");
    let (stdout, _, ok) = run(&dotfiles, &["which", gitconfig.to_str().unwrap()]);
    assert!(ok);
    assert!(stdout.contains("package: git"), "{stdout}");
    assert!(stdout.contains("via:     config"), "{stdout}");
    assert!(stdout.contains("stale"), "{stdout}");
}

#[test]
fn test_which_exits_nonzero_for_unmanaged_path() {
    let tmp = TempDir::new().unwrap();
    setup_dotfiles(&tmp);
    let dotfiles = tmp.path().join("dotfiles");
    let stray = tmp.path().join("home/.bashrc");
    fs::write(&stray, "# not ours").unwrap();

    let (stdout, _, ok) = run(&dotfiles, &["which", stray.to_str().unwrap()]);
    assert!(!ok, "expected a nonzero exit for an unmanaged path");
    assert!(stdout.contains("not managed by notfiles"), "{stdout}");
}

