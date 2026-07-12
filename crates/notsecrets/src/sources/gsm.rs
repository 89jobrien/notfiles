use crate::config::{Provider, SecretRef};
use crate::error::SecretsError;
use crate::ports::SecretSource;
use std::process::Command;

pub struct GsmSource {
    project: String,
}

impl GsmSource {
    pub fn new(project: String) -> Self {
        Self { project }
    }
}

impl SecretSource for GsmSource {
    fn name(&self) -> &str {
        "gsm"
    }

    fn provider(&self) -> Provider {
        Provider::Gsm
    }

    fn resolve(&self, _key: &str) -> Result<Option<String>, SecretsError> {
        Ok(None)
    }

    fn resolve_ref(
        &self,
        key: &str,
        secret_ref: &SecretRef,
    ) -> Result<Option<String>, SecretsError> {
        let SecretRef::Gsm { path } = secret_ref else {
            return self.resolve(key);
        };
        let output = Command::new("gcloud")
            .args([
                "secrets",
                "versions",
                "access",
                "latest",
                "--secret",
                path,
                "--project",
                &self.project,
            ])
            .output()
            .map_err(|e| SecretsError::SourceError {
                name: "gsm".to_string(),
                source: e.into(),
            })?;
        if output.status.success() {
            Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            ))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(SecretsError::SourceError {
                name: "gsm".to_string(),
                source: anyhow::anyhow!("gcloud secrets failed: {stderr}"),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gsm_source_name_and_provider() {
        let source = GsmSource::new("my-project".to_string());
        assert_eq!(source.name(), "gsm");
        assert_eq!(source.provider(), Provider::Gsm);
    }

    #[test]
    fn gsm_source_resolve_without_ref_returns_none() {
        let source = GsmSource::new("my-project".to_string());
        let result = source.resolve("KEY").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn gsm_source_resolve_ref_wrong_variant_delegates() {
        let source = GsmSource::new("my-project".to_string());
        let wrong_ref = SecretRef::Env;
        let result = source.resolve_ref("KEY", &wrong_ref).unwrap();
        assert_eq!(result, None);
    }
}
