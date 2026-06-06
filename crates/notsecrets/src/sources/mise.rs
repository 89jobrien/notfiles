use crate::config::Provider;
use crate::error::SecretsError;
use crate::ports::SecretSource;

pub struct MiseSource;

impl SecretSource for MiseSource {
    fn name(&self) -> &str {
        "mise"
    }

    fn provider(&self) -> Provider {
        Provider::Mise
    }

    fn resolve(&self, _key: &str) -> Result<Option<String>, SecretsError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mise_stub_resolve_returns_none() {
        assert_eq!(MiseSource.resolve("ANY").unwrap(), None);
    }

    #[test]
    fn mise_stub_name_and_provider() {
        assert_eq!(MiseSource.name(), "mise");
        assert_eq!(MiseSource.provider(), Provider::Mise);
    }
}
