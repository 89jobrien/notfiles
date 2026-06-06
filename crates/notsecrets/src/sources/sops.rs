use crate::config::Provider;
use crate::error::SecretsError;
use crate::ports::{EnumerableSecretSource, SecretSource};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

pub struct SopsSource {
    file: PathBuf,
}

impl SopsSource {
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }

    fn decrypt_all(&self) -> Result<HashMap<String, String>, SecretsError> {
        let output = Command::new("sops")
            .args(["--decrypt", &self.file.to_string_lossy()])
            .output()
            .map_err(|e| SecretsError::SourceError {
                name: "sops".to_string(),
                source: e.into(),
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SecretsError::SourceError {
                name: "sops".to_string(),
                source: anyhow::anyhow!("sops decrypt failed: {stderr}"),
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut map = HashMap::new();
        for line in stdout.lines() {
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                if !k.is_empty() && !k.starts_with('#') {
                    map.insert(k.to_string(), v.to_string());
                }
            }
        }
        Ok(map)
    }
}

impl SecretSource for SopsSource {
    fn name(&self) -> &str {
        "sops"
    }

    fn provider(&self) -> Provider {
        Provider::Sops
    }

    fn resolve(&self, key: &str) -> Result<Option<String>, SecretsError> {
        self.decrypt_all().map(|m| m.get(key).cloned())
    }
}

impl EnumerableSecretSource for SopsSource {
    fn resolve_all(&self) -> Result<HashMap<String, String>, SecretsError> {
        self.decrypt_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sops_source_name_and_provider() {
        let source = SopsSource::new("/tmp/secrets.sops.env".into());
        assert_eq!(source.name(), "sops");
        assert_eq!(source.provider(), Provider::Sops);
    }

    #[test]
    fn sops_source_resolve_missing_file_returns_error() {
        let source = SopsSource::new("/nonexistent/secrets.sops.env".into());
        let result = source.resolve("KEY");
        assert!(result.is_err());
    }
}
