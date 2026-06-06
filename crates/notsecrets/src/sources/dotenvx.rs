use crate::config::Provider;
use crate::error::SecretsError;
use crate::ports::{EnumerableSecretSource, SecretSource};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

pub struct DotenvxSource {
    env_file: PathBuf,
}

impl DotenvxSource {
    pub fn new(env_file: PathBuf) -> Self {
        Self { env_file }
    }

    fn decrypt_all(&self) -> Result<HashMap<String, String>, SecretsError> {
        let output = Command::new("dotenvx")
            .args([
                "run",
                "--env-file",
                &self.env_file.to_string_lossy(),
                "--",
                "env",
            ])
            .output()
            .map_err(|e| SecretsError::SourceError {
                name: "dotenvx".to_string(),
                source: e.into(),
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SecretsError::SourceError {
                name: "dotenvx".to_string(),
                source: anyhow::anyhow!("dotenvx run failed: {stderr}"),
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

impl SecretSource for DotenvxSource {
    fn name(&self) -> &str {
        "dotenvx"
    }

    fn provider(&self) -> Provider {
        Provider::Dotenvx
    }

    fn resolve(&self, key: &str) -> Result<Option<String>, SecretsError> {
        self.decrypt_all().map(|m| m.get(key).cloned())
    }
}

impl EnumerableSecretSource for DotenvxSource {
    fn resolve_all(&self) -> Result<HashMap<String, String>, SecretsError> {
        self.decrypt_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotenvx_source_name_and_provider() {
        let source = DotenvxSource::new("/tmp/test.env".into());
        assert_eq!(source.name(), "dotenvx");
        assert_eq!(source.provider(), Provider::Dotenvx);
    }

    #[test]
    fn dotenvx_source_resolve_missing_file_returns_error() {
        let source = DotenvxSource::new("/nonexistent/.env".into());
        let result = source.resolve_all();
        assert!(result.is_err());
    }
}
