use std::process::Command;

use crate::error::NotforgeError;
use crate::ports::{GitRemoteManager, LocalRepository, PushStatus, RemoteStatus};

/// [`GitRemoteManager`] implemented by shelling out to the `git` binary.
///
/// Every invocation uses discrete `.arg()` calls with no shell involved,
/// mirroring the pattern already audited in
/// `notstrap::repo::clone_if_missing`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CommandGitRemoteManager;

impl GitRemoteManager for CommandGitRemoteManager {
    fn remote_url(
        &self,
        repo: &LocalRepository,
        remote_name: &str,
    ) -> Result<Option<String>, NotforgeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&repo.path)
            .arg("remote")
            .arg("get-url")
            .arg(remote_name)
            .output()
            .map_err(|e| NotforgeError::Git(format!("failed to run git remote get-url: {e}")))?;

        if !output.status.success() {
            return Ok(None);
        }
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(Some(url))
    }

    fn set_remote(
        &self,
        repo: &LocalRepository,
        remote_name: &str,
        url: &str,
    ) -> Result<RemoteStatus, NotforgeError> {
        match self.remote_url(repo, remote_name)? {
            None => {
                run_git(
                    &repo.path,
                    &["remote", "add", remote_name, url],
                    "git remote add",
                )?;
                Ok(RemoteStatus::Added)
            }
            Some(existing) if existing == url => Ok(RemoteStatus::Unchanged),
            Some(_) => {
                run_git(
                    &repo.path,
                    &["remote", "set-url", remote_name, url],
                    "git remote set-url",
                )?;
                Ok(RemoteStatus::Updated)
            }
        }
    }

    fn push(
        &self,
        repo: &LocalRepository,
        remote_name: &str,
        branch: &str,
        set_upstream: bool,
    ) -> Result<PushStatus, NotforgeError> {
        let mut args = vec!["push"];
        if set_upstream {
            args.push("--set-upstream");
        }
        args.push(remote_name);
        args.push(branch);

        let output = Command::new("git")
            .arg("-C")
            .arg(&repo.path)
            .args(&args)
            .output()
            .map_err(|e| NotforgeError::Git(format!("failed to run git push: {e}")))?;

        if !output.status.success() {
            return Err(NotforgeError::Git(format!(
                "git push failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Everything up-to-date") {
            Ok(PushStatus::AlreadyUpToDate)
        } else {
            Ok(PushStatus::Pushed)
        }
    }
}

fn run_git(repo_path: &std::path::Path, args: &[&str], action: &str) -> Result<(), NotforgeError> {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .status()
        .map_err(|e| NotforgeError::Git(format!("failed to run {action}: {e}")))?;

    if !status.success() {
        return Err(NotforgeError::Git(format!("{action} failed")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn init_repo() -> (tempfile::TempDir, LocalRepository) {
        let dir = tempfile::TempDir::new().unwrap();
        let path: PathBuf = dir.path().to_path_buf();
        run_git(&path, &["init"], "git init").unwrap();
        run_git(
            &path,
            &["config", "user.email", "test@example.com"],
            "git config",
        )
        .unwrap();
        run_git(&path, &["config", "user.name", "Test"], "git config").unwrap();
        (dir, LocalRepository { path })
    }

    #[test]
    fn remote_url_returns_none_when_absent() {
        let (_dir, repo) = init_repo();
        let manager = CommandGitRemoteManager;
        assert_eq!(manager.remote_url(&repo, "origin").unwrap(), None);
    }

    #[test]
    fn set_remote_adds_when_missing() {
        let (_dir, repo) = init_repo();
        let manager = CommandGitRemoteManager;

        let status = manager
            .set_remote(&repo, "origin", "https://example.com/x.git")
            .unwrap();

        assert_eq!(status, RemoteStatus::Added);
        assert_eq!(
            manager.remote_url(&repo, "origin").unwrap(),
            Some("https://example.com/x.git".to_string())
        );
    }

    #[test]
    fn set_remote_is_unchanged_when_url_matches() {
        let (_dir, repo) = init_repo();
        let manager = CommandGitRemoteManager;

        manager
            .set_remote(&repo, "origin", "https://example.com/x.git")
            .unwrap();
        let status = manager
            .set_remote(&repo, "origin", "https://example.com/x.git")
            .unwrap();

        assert_eq!(status, RemoteStatus::Unchanged);
    }

    #[test]
    fn set_remote_updates_when_url_differs() {
        let (_dir, repo) = init_repo();
        let manager = CommandGitRemoteManager;

        manager
            .set_remote(&repo, "origin", "https://example.com/x.git")
            .unwrap();
        let status = manager
            .set_remote(&repo, "origin", "https://example.com/y.git")
            .unwrap();

        assert_eq!(status, RemoteStatus::Updated);
        assert_eq!(
            manager.remote_url(&repo, "origin").unwrap(),
            Some("https://example.com/y.git".to_string())
        );
    }
}
