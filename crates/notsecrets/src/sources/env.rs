use crate::config::Provider;
use crate::error::SecretsError;
use crate::ports::{EnumerableSecretSource, SecretSource};
use std::collections::HashMap;

pub struct EnvSource;

impl SecretSource for EnvSource {
    fn name(&self) -> &str {
        "env"
    }

    fn provider(&self) -> Provider {
        Provider::Env
    }

    fn resolve(&self, key: &str) -> Result<Option<String>, SecretsError> {
        Ok(std::env::var(key).ok())
    }
}

impl EnumerableSecretSource for EnvSource {
    fn resolve_all(&self) -> Result<HashMap<String, String>, SecretsError> {
        Ok(std::env::vars().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_source_resolve_existing_key() {
        // SAFETY: test is single-threaded
        unsafe { std::env::set_var("NOTSECRETS_TEST_KEY", "test_value") };
        let source = EnvSource;
        let result = source.resolve("NOTSECRETS_TEST_KEY").unwrap();
        assert_eq!(result, Some("test_value".to_string()));
        unsafe { std::env::remove_var("NOTSECRETS_TEST_KEY") };
    }

    #[test]
    fn env_source_resolve_missing_key() {
        let source = EnvSource;
        let result = source.resolve("NOTSECRETS_NONEXISTENT_KEY_12345").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn env_source_resolve_all_contains_home_or_user() {
        let source = EnvSource;
        let map = source.resolve_all().unwrap();
        assert!(map.contains_key("HOME") || map.contains_key("USER"));
    }

    #[test]
    fn env_source_name_and_provider() {
        let source = EnvSource;
        assert_eq!(source.name(), "env");
        assert_eq!(source.provider(), Provider::Env);
    }
}
