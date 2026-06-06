use crate::config::{Provider, SecretRef};
use crate::error::SecretsError;
use crate::ports::SecretSource;
use std::process::Command;

pub struct OpSource {
    account: String,
}

impl OpSource {
    pub fn new(account: String) -> Self {
        Self { account }
    }
}

impl SecretSource for OpSource {
    fn name(&self) -> &str {
        "op"
    }

    fn provider(&self) -> Provider {
        Provider::Op
    }

    /// OpSource cannot resolve by key name alone -- requires a ref.
    fn resolve(&self, _key: &str) -> Result<Option<String>, SecretsError> {
        Ok(None)
    }

    fn resolve_ref(
        &self,
        key: &str,
        secret_ref: &SecretRef,
    ) -> Result<Option<String>, SecretsError> {
        let SecretRef::Op { uri } = secret_ref else {
            return self.resolve(key);
        };
        let output = Command::new("op")
            .args(["read", uri, "--account", &self.account])
            .output()
            .map_err(|e| SecretsError::SourceError {
                name: "op".to_string(),
                source: e.into(),
            })?;
        if output.status.success() {
            Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            ))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(SecretsError::SourceError {
                name: "op".to_string(),
                source: anyhow::anyhow!("op read failed: {stderr}"),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_source_name_and_provider() {
        let source = OpSource::new("my.1password.com".to_string());
        assert_eq!(source.name(), "op");
        assert_eq!(source.provider(), Provider::Op);
    }

    #[test]
    fn op_source_resolve_without_ref_returns_none() {
        let source = OpSource::new("my.1password.com".to_string());
        let result = source.resolve("SOME_KEY").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn op_source_resolve_ref_wrong_variant_delegates() {
        let source = OpSource::new("my.1password.com".to_string());
        let wrong_ref = SecretRef::Env;
        let result = source.resolve_ref("KEY", &wrong_ref).unwrap();
        assert_eq!(result, None);
    }
}
