use crate::config::Provider;
use crate::error::SecretsError;
use crate::ports::SecretSource;

pub struct NuenvSource;

impl SecretSource for NuenvSource {
    fn name(&self) -> &str {
        "nuenv"
    }

    fn provider(&self) -> Provider {
        Provider::Nuenv
    }

    fn resolve(&self, _key: &str) -> Result<Option<String>, SecretsError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nuenv_stub_resolve_returns_none() {
        let source = NuenvSource;
        assert_eq!(source.resolve("ANY").unwrap(), None);
    }

    #[test]
    fn nuenv_stub_name_and_provider() {
        let source = NuenvSource;
        assert_eq!(source.name(), "nuenv");
        assert_eq!(source.provider(), Provider::Nuenv);
    }
}
