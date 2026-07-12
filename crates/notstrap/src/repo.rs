use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

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
