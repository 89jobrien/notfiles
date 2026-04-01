pub mod sources;

pub use sources::{BitwardenSource, FileSource, PromptSource};

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Trait for retrieving an age private key from some source.
pub trait AgeKeySource {
    fn name(&self) -> &str;
    fn retrieve(&self) -> Result<String>;
}

/// Try each source in order; return the first success.
pub fn resolve_age_key(sources: Vec<Box<dyn AgeKeySource>>) -> Result<String> {
    let mut last_err = String::new();
    for source in sources {
        match source.retrieve() {
            Ok(key) => return Ok(key),
            Err(e) => {
                eprintln!("  [{}] {e}", source.name());
                last_err = format!("{e}");
            }
        }
    }
    bail!("all age key sources failed; last error: {last_err}")
}

/// Write the age key to `~/.config/sops/age/keys.txt` (mode 0600).
pub fn install_age_key(key: &str) -> Result<PathBuf> {
    let path = dirs::home_dir()
        .context("cannot find home directory")?
        .join(".config/sops/age/keys.txt");
    std::fs::create_dir_all(path.parent().with_context(|| "age keys.txt path has no parent directory")?)?;
    std::fs::write(&path, key)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(path)
}

/// Run `sops --decrypt <sops_file>` and return the decrypted content.
pub fn decrypt_sops(sops_file: &Path) -> Result<String> {
    let output = Command::new("sops")
        .args(["--decrypt", sops_file.to_str().ok_or_else(|| anyhow::anyhow!("sops path is not valid UTF-8"))?])
        .output()
        .context("failed to run sops")?;

    if !output.status.success() {
        bail!("sops decrypt failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(String::from_utf8(output.stdout)?)
}
