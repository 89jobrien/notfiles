use crate::config::{Provider, SecretRef};
use crate::error::{AgeError, SecretsError};
use crate::identities::Identity;
use std::collections::HashMap;

/// Infra boundary trait: an identity resolver that loads key material from an
/// external source and returns a concrete Identity.
pub trait IdentitySource {
    /// Human-readable name of the identity source (e.g., "bitwarden", "file", "prompt").
    fn name(&self) -> &str;

    /// Load and return a concrete identity from the source.
    fn load(&self) -> Result<Box<dyn Identity>, AgeError>;
}

/// Port: resolve a single secret by key name from an external provider.
pub trait SecretSource {
    fn name(&self) -> &str;
    fn provider(&self) -> Provider;
    fn resolve(&self, key: &str) -> Result<Option<String>, SecretsError>;

    /// Resolve using a provider-specific typed reference.
    /// Default: ignores ref, falls back to resolve(key).
    fn resolve_ref(
        &self,
        key: &str,
        _secret_ref: &SecretRef,
    ) -> Result<Option<String>, SecretsError> {
        self.resolve(key)
    }
}

/// Extended port: sources that can enumerate all available secrets.
pub trait EnumerableSecretSource: SecretSource {
    fn resolve_all(&self) -> Result<HashMap<String, String>, SecretsError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSource;
    impl SecretSource for FakeSource {
        fn name(&self) -> &str {
            "fake"
        }
        fn provider(&self) -> Provider {
            Provider::Env
        }
        fn resolve(&self, _key: &str) -> Result<Option<String>, SecretsError> {
            Ok(Some("val".to_string()))
        }
    }

    #[test]
    fn default_resolve_ref_delegates_to_resolve() {
        let source = FakeSource;
        let secret_ref = SecretRef::Env;
        let result = source.resolve_ref("KEY", &secret_ref).unwrap();
        assert_eq!(result, Some("val".to_string()));
    }
}
