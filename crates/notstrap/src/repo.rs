use anyhow::{Context, Result};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Boundary for fetching a repository.
///
/// Contract: `Ok` means `dest` now holds a checkout of `url`; any failure is `Err`.
pub trait GitClone {
    fn clone_repo(&self, url: &str, dest: &Path) -> Result<()>;
}

/// Clones with the system `git` binary.
pub struct SystemGit;

impl GitClone for SystemGit {
    fn clone_repo(&self, url: &str, dest: &Path) -> Result<()> {
        // `url` comes from the notstrap.toml config and `dest` is a `Path` derived from the
        // resolved dotfiles directory. Both are passed as discrete `.arg()` calls with no
        // shell involved, so argument injection is not possible.
        let status = Command::new("git")
            .arg("clone")
            .arg(url)
            .arg(dest)
            .status()
            .context("failed to run git clone")?;
        if !status.success() {
            anyhow::bail!("git clone {} failed", url);
        }
        Ok(())
    }
}

/// Test double for `GitClone`: records `(url, dest)` and optionally fails every call.
#[derive(Default)]
pub struct FakeGit {
    error: Option<String>,
    calls: RefCell<Vec<(String, PathBuf)>>,
}

impl FakeGit {
    pub fn failing(message: &str) -> Self {
        Self {
            error: Some(message.to_string()),
            calls: RefCell::default(),
        }
    }

    pub fn calls(&self) -> Vec<(String, PathBuf)> {
        self.calls.borrow().clone()
    }
}

impl GitClone for FakeGit {
    fn clone_repo(&self, url: &str, dest: &Path) -> Result<()> {
        self.calls
            .borrow_mut()
            .push((url.to_string(), dest.to_path_buf()));
        match &self.error {
            Some(message) => anyhow::bail!("{message}"),
            None => Ok(()),
        }
    }
}

/// Clone `url` into `dest` unless `dest` already exists. `Ok(true)` means a clone happened.
pub fn clone_if_missing(git: &dyn GitClone, url: &str, dest: &Path) -> Result<bool> {
    if dest.exists() {
        return Ok(false);
    }
    git.clone_repo(url, dest)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn existing_destination_is_not_cloned() {
        let dir = tempfile::tempdir().expect("tempdir");
        let git = FakeGit::default();

        let cloned = clone_if_missing(&git, "https://example/dots.git", dir.path())
            .expect("no clone needed");

        assert!(!cloned);
        assert!(git.calls().is_empty());
    }

    #[test]
    fn missing_destination_is_cloned_from_the_url() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dest = dir.path().join("dots");
        let git = FakeGit::default();

        let cloned = clone_if_missing(&git, "https://example/dots.git", &dest).expect("clone");

        assert!(cloned);
        assert_eq!(
            git.calls(),
            [("https://example/dots.git".to_string(), dest)]
        );
    }

    #[test]
    fn clone_failure_is_propagated() {
        let dir = tempfile::tempdir().expect("tempdir");
        let git = FakeGit::failing("network down");

        let err = clone_if_missing(&git, "u", &dir.path().join("dots")).expect_err("fails");

        assert!(err.to_string().contains("network down"));
    }

    #[test]
    fn fake_records_destinations_as_paths() {
        let git = FakeGit::default();
        git.clone_repo("u", &PathBuf::from("/x"))
            .expect("fake succeeds");
        assert_eq!(git.calls()[0].1, PathBuf::from("/x"));
    }
}
