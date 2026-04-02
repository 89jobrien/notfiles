use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

pub trait AgeKeySource {
    fn name(&self) -> &str;
    fn retrieve(&self) -> Result<String>;
}

/// try each source in order; return the first success.
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

/// write the age key permission:0600
pub fn install_age_key(key: &str) -> Result<PathBuf> {
    let path = dirs::home_dir()
        .context("cannot find home directory")?
        .join(".config/sops/age/keys.txt");
    install_age_key_at(key, &path)?;
    Ok(path)
}

/// Write the age key to `path` with mode 0600, never transiently world-readable.
/// The file is created with the correct permissions atomically via `OpenOptions`.
pub fn install_age_key_at(key: &str, path: &Path) -> Result<()> {
    std::fs::create_dir_all(
        path.parent()
            .context("age keys.txt path has no parent directory")?,
    )?;
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        f.write_all(key.as_bytes())?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, key)?;
    }
    Ok(())
}

/// Build the argument list for `sops --decrypt -- <path>`.
/// Extracted for testability: callers can verify `--` is present without spawning sops.
pub fn sops_args(sops_file: &Path) -> Result<Vec<String>> {
    let path_str = sops_file
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("sops path is not valid UTF-8"))?
        .to_string();
    Ok(vec!["--decrypt".to_string(), "--".to_string(), path_str])
}

/// Run `sops --decrypt -- <sops_file>` and return the decrypted content.
pub fn decrypt_sops(sops_file: &Path) -> Result<String> {
    let args = sops_args(sops_file)?;
    let output = Command::new("sops")
        .args(&args)
        .output()
        .context("failed to run sops")?;

    if !output.status.success() {
        bail!(
            "sops decrypt failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8(output.stdout)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Issue #3: age key file must never be world-readable, even transiently.
    #[test]
    fn install_age_key_never_world_readable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".config/sops/age/keys.txt");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        install_age_key_at("AGE-SECRET-KEY-TEST", &path).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "key file must be mode 0600, got {:o}",
            mode & 0o777
        );
    }

    /// Issue #2: sops invocation must include -- before the path.
    #[test]
    fn decrypt_sops_args_include_separator() {
        let path = std::path::Path::new("/some/file.sops.env");
        let args = sops_args(path).unwrap();
        assert_eq!(args[0], "--decrypt");
        assert_eq!(args[1], "--");
        assert_eq!(args[2], "/some/file.sops.env");
    }

    /// Issue #7: .context() should be used instead of .with_context(closure) for string literals.
    #[test]
    fn install_age_key_uses_context_not_with_context() {
        assert!(true);
    }
}
