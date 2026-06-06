use crate::config::Provider;
use crate::error::SecretsError;
use crate::ports::SecretSource;

pub struct DirenvSource;

impl SecretSource for DirenvSource {
    fn name(&self) -> &str {
        "direnv"
    }

    fn provider(&self) -> Provider {
        Provider::Direnv
    }

    fn resolve(&self, _key: &str) -> Result<Option<String>, SecretsError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direnv_stub_resolve_returns_none() {
        assert_eq!(DirenvSource.resolve("ANY").unwrap(), None);
    }

    #[test]
    fn direnv_stub_name_and_provider() {
        assert_eq!(DirenvSource.name(), "direnv");
        assert_eq!(DirenvSource.provider(), Provider::Direnv);
    }
}
