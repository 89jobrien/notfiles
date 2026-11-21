use notstrap::repo::{GitClone, SystemGit, clone_if_missing};
use std::process::Command;
use tempfile::TempDir;

fn init_origin(dir: &TempDir) -> std::path::PathBuf {
    let origin = dir.path().join("origin");
    std::fs::create_dir(&origin).expect("mkdir origin");
    for args in [
        vec!["init", "-q"],
        vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "init",
        ],
    ] {
        let status = Command::new("git")
            .args(&args)
            .current_dir(&origin)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?} failed");
    }
    origin
}

#[test]
fn system_git_clones_a_local_repository() {
    let dir = TempDir::new().expect("tempdir");
    let origin = init_origin(&dir);
    let dest = dir.path().join("checkout");

    SystemGit
        .clone_repo(origin.to_str().expect("utf8 path"), &dest)
        .expect("clone");

    assert!(dest.join(".git").exists());
}

#[test]
fn system_git_reports_a_failed_clone_as_err() {
    let dir = TempDir::new().expect("tempdir");
    let missing = dir.path().join("no-such-origin");

    let result = SystemGit.clone_repo(
        missing.to_str().expect("utf8 path"),
        &dir.path().join("checkout"),
    );

    assert!(result.is_err());
}

#[test]
fn clone_if_missing_is_idempotent_over_system_git() {
    let dir = TempDir::new().expect("tempdir");
    let origin = init_origin(&dir);
    let dest = dir.path().join("checkout");
    let url = origin.to_str().expect("utf8 path");

    assert!(clone_if_missing(&SystemGit, url, &dest).expect("first"));
    assert!(!clone_if_missing(&SystemGit, url, &dest).expect("second"));
}
