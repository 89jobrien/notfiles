use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub fn clone_if_missing(url: &str, dest: &Path) -> Result<bool> {
    if dest.exists() {
        return Ok(false);
    }
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
