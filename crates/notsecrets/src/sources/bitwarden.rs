use crate::error::AgeError;
use crate::identities::Identity;
use crate::identities::x25519::X25519Identity;
use crate::sources::IdentitySource;
use std::io::Write;
use std::process::{Command, Stdio};

pub struct BitwardenSource {
    pub item_name: String,
}

impl BitwardenSource {
    pub fn new(item_name: impl Into<String>) -> Self {
        Self {
            item_name: item_name.into(),
        }
    }

    fn retrieve_key(&self) -> Result<String, AgeError> {
        let bw_ok = Command::new("sh")
            .args(["-c", "command -v bw"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !bw_ok {
            return Err(AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("bw CLI not found in PATH"),
            });
        }

        let session = std::env::var("BW_SESSION").unwrap_or_default();
        let session = if session.is_empty() {
            let password =
                rpassword::prompt_password("Bitwarden master password: ").map_err(|e| {
                    AgeError::SourceError {
                        name: self.name().to_string(),
                        source: anyhow::anyhow!("could not read password: {e}"),
                    }
                })?;
            // Pass password via stdin to avoid it appearing in the process list.
            let mut child = Command::new("bw")
                .args(["unlock", "--raw", "--passwordstdin"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| AgeError::SourceError {
                    name: self.name().to_string(),
                    source: anyhow::anyhow!("bw unlock spawn: {e}"),
                })?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(password.as_bytes())
                    .map_err(|e| AgeError::SourceError {
                        name: self.name().to_string(),
                        source: anyhow::anyhow!("bw unlock stdin write: {e}"),
                    })?;
            }
            let output = child
                .wait_with_output()
                .map_err(|e| AgeError::SourceError {
                    name: self.name().to_string(),
                    source: anyhow::anyhow!("bw unlock wait: {e}"),
                })?;
            if !output.status.success() {
                return Err(AgeError::SourceError {
                    name: self.name().to_string(),
                    source: anyhow::anyhow!(
                        "bw unlock failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ),
                });
            }
            String::from_utf8(output.stdout)
                .map_err(|e| AgeError::SourceError {
                    name: self.name().to_string(),
                    source: anyhow::anyhow!("bw unlock output UTF-8: {e}"),
                })?
                .trim()
                .to_string()
        } else {
            session
        };

        let output = Command::new("bw")
            .args(["get", "notes", &self.item_name, "--session", &session])
            .output()
            .map_err(|e| AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("bw get spawn: {e}"),
            })?;
        if !output.status.success() {
            return Err(AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!(
                    "bw get notes '{}' failed: {}",
                    self.item_name,
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }
        let key = String::from_utf8(output.stdout)
            .map_err(|e| AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("bw output UTF-8: {e}"),
            })?
            .trim()
            .to_string();
        if key.is_empty() {
            return Err(AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("Bitwarden item '{}' has empty notes", self.item_name),
            });
        }
        Ok(key)
    }
}

impl IdentitySource for BitwardenSource {
    fn name(&self) -> &str {
        "bitwarden"
    }

    fn load(&self) -> Result<Box<dyn Identity>, AgeError> {
        let key = self.retrieve_key()?;
        let identity =
            X25519Identity::from_bech32(key.trim()).map_err(|e| AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("key is not a valid AGE-SECRET-KEY-1: {e}"),
            })?;
        Ok(Box::new(identity))
    }
}
