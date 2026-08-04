use std::fs;
use std::process::Command;

use notcore::Config;
use notfiles::linker::{LinkOptions, State};
use notfiles::{FileStore, FileStoreImpl, doctor, link_with_store};
use tempfile::TempDir;

/// Real end-to-end doctor run: a linked package whose source later
/// disappears (orphan) plus an untracked file in the dotfiles git repo
/// (dirty state) should both surface as issues.
#[test]
fn doctor_reports_orphan_and_dirty_repo() {
    let dotfiles = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let d = dotfiles.path();

    fs::write(
        d.join("notfiles.toml"),
        format!(
            "[defaults]\nmethod = \"symlink\"\ntarget = \"{}\"\n",
            home.path().display()
        ),
    )
    .unwrap();

    let pkg = d.join("shell");
    fs::create_dir_all(&pkg).unwrap();
    let source = pkg.join(".zshrc");
    fs::write(&source, "# test zshrc\n").unwrap();

    // Link it for real, then delete the source — the target symlink and
    // state entry remain, so `status::package_status` classifies it Orphan.
    let fs_impl = &FileStoreImpl;
    let opts = LinkOptions {
        force: false,
        no_backup: false,
        dry_run: false,
        verbose: false,
    };
    link_with_store(
        d,
        &[],
        &opts,
        fs_impl,
        &notfiles::adapters::TerminalReporter,
    )
    .unwrap();
    fs::remove_file(&source).unwrap();

    // Untracked dotfiles.toml/package in a freshly-initialized git repo ->
    // `git status --porcelain` is non-empty -> dirty.
    let status = Command::new("git")
        .arg("-C")
        .arg(d)
        .arg("init")
        .arg("--quiet")
        .status()
        .expect("git must be on PATH for this test");
    assert!(status.success());

    let config = Config::load_from(&d.join("notfiles.toml")).unwrap();
    let state = State::load(d, fs_impl).unwrap();

    let report = doctor::run(d, &config, &state, fs_impl);

    assert!(
        report.issues.iter().any(|i| matches!(
            i,
            doctor::DoctorIssue::Orphan { package, .. } if package == "shell"
        )),
        "expected an Orphan issue for package `shell`, got: {:?}",
        report.issues
    );
    assert!(
        report
            .issues
            .iter()
            .any(|i| matches!(i, doctor::DoctorIssue::DirtyGit)),
        "expected a DirtyGit issue, got: {:?}",
        report.issues
    );
    assert!(!report.is_clean());

    // Sanity: the FileStore port is reachable through the same handle used
    // by doctor — confirms the crate's public re-exports still line up.
    assert!(fs_impl.exists(d));
}
