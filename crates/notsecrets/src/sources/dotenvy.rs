use crate::config::Provider;
use crate::error::SecretsError;
use crate::ports::SecretSource;

pub struct DotenvySource;

impl SecretSource for DotenvySource {
    fn name(&self) -> &str {
        "dotenvy"
    }

    fn provider(&self) -> Provider {
        Provider::Dotenvy
    }

    fn resolve(&self, _key: &str) -> Result<Option<String>, SecretsError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotenvy_stub_resolve_returns_none() {
        assert_eq!(DotenvySource.resolve("ANY").unwrap(), None);
    }

    #[test]
    fn dotenvy_stub_name_and_provider() {
        assert_eq!(DotenvySource.name(), "dotenvy");
        assert_eq!(DotenvySource.provider(), Provider::Dotenvy);
    }
}
