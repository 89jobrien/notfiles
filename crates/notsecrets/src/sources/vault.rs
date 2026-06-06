use crate::config::Provider;
use crate::error::SecretsError;
use crate::ports::SecretSource;

pub struct VaultSource;

impl SecretSource for VaultSource {
    fn name(&self) -> &str {
        "vault"
    }

    fn provider(&self) -> Provider {
        Provider::Vault
    }

    fn resolve(&self, _key: &str) -> Result<Option<String>, SecretsError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_stub_resolve_returns_none() {
        assert_eq!(VaultSource.resolve("ANY").unwrap(), None);
    }

    #[test]
    fn vault_stub_name_and_provider() {
        assert_eq!(VaultSource.name(), "vault");
        assert_eq!(VaultSource.provider(), Provider::Vault);
    }
}
