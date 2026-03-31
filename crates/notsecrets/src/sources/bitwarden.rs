use anyhow::{bail, Result};
use std::process::Command;
use which::which;
use crate::AgeKeySource;

pub struct BitwardenSource {
    pub item_name: String,
}

impl BitwardenSource {
    pub fn new(item_name: impl Into<String>) -> Self {
        Self { item_name: item_name.into() }
    }
}

impl AgeKeySource for BitwardenSource {
    fn name(&self) -> &str { "bitwarden" }

    fn retrieve(&self) -> Result<String> {
        if which("bw").is_err() {
            bail!("bw CLI not found in PATH");
        }

        let session = std::env::var("BW_SESSION").unwrap_or_default();
        let session = if session.is_empty() {
            let password = rpassword::prompt_password("Bitwarden master password: ")
                .map_err(|e| anyhow::anyhow!("could not read password: {e}"))?;
            let output = Command::new("bw")
                .args(["unlock", "--raw", &password])
                .output()?;
            if !output.status.success() {
                bail!("bw unlock failed: {}", String::from_utf8_lossy(&output.stderr));
            }
            String::from_utf8(output.stdout)?.trim().to_string()
        } else {
            session
        };

        let output = Command::new("bw")
            .args(["get", "notes", &self.item_name, "--session", &session])
            .output()?;

        if !output.status.success() {
            bail!(
                "bw get notes '{}' failed: {}",
                self.item_name,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let key = String::from_utf8(output.stdout)?.trim().to_string();
        if key.is_empty() {
            bail!("Bitwarden item '{}' has empty notes", self.item_name);
        }
        Ok(key)
    }
}
