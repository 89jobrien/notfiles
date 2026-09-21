//! Clones the configured dotfiles repository when its destination is absent.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Clones `url` into `dest`, returning false when `dest` already exists.
pub fn clone_if_missing(url: &str, dest: &Path) -> Result<bool> {
    if dest.exists() {
        return Ok(false);
    }
    // SAFETY: `url` comes from the notstrap.toml config and `dest` is a `Path` derived from
    // the resolved dotfiles directory. Both are passed as discrete `.arg()` calls with no
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
    Ok(true)
}
