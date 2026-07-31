use notforge::config::ForgeSecretRef;
use notforge::error::NotforgeError;
use notforge::ports::SecretResolverPort;

/// Bridges [`notforge::ports::SecretResolverPort`] to an existing
/// [`notsecrets::SecretResolver`], so `notforge` never depends on
/// `notsecrets` directly.
///
/// `ForgeSecretRef::Env` resolves via `notsecrets`'s name-based priority
/// chain. `ForgeSecretRef::Op` is not yet supported: `notsecrets::SecretResolver::resolve`
/// only resolves 1Password references through a name bound in
/// `notsecrets.toml`'s `[secrets]` table (`SecretRef::Op { uri }`), not
/// an ad hoc URI supplied at call time — there is no public API on
/// `SecretResolver` today to resolve an unbound `op://` URI directly.
pub struct NotsecretsResolverAdapter<'a> {
    resolver: &'a notsecrets::SecretResolver,
}

impl<'a> NotsecretsResolverAdapter<'a> {
    /// Wrap an existing `notsecrets::SecretResolver`.
    pub fn new(resolver: &'a notsecrets::SecretResolver) -> Self {
        Self { resolver }
    }
}

impl SecretResolverPort for NotsecretsResolverAdapter<'_> {
    fn resolve(&self, secret: &ForgeSecretRef) -> Result<String, NotforgeError> {
        match secret {
            ForgeSecretRef::Env { key } => self
                .resolver
                .resolve(key)
                .map_err(|e| NotforgeError::Secret(e.to_string()))?
                .ok_or_else(|| NotforgeError::Secret(format!("secret '{key}' not found"))),
            ForgeSecretRef::Op { uri } => Err(NotforgeError::Secret(format!(
                "op:// forge secrets are not yet supported ('{uri}'); bind the secret in \
                 notsecrets.toml's [secrets] table under a name and reference it via \
                 ForgeSecretRef::Env instead"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notsecrets::SecretResolver;
    use notsecrets::config::{Provider, SecretsConfig};
    use std::collections::HashMap;

    fn env_only_resolver() -> SecretResolver {
        SecretResolver::from_config(SecretsConfig {
            providers: vec![Provider::Env],
            provider: HashMap::new(),
            secrets: HashMap::new(),
        })
        .unwrap()
    }

    #[test]
    fn adapter_resolves_via_notsecrets_env_source() {
        unsafe { std::env::set_var("NOTFORGE_ADAPTER_TEST", "resolved-value") };
        let resolver = env_only_resolver();
        let adapter = NotsecretsResolverAdapter::new(&resolver);

        let value = adapter
            .resolve(&ForgeSecretRef::Env {
                key: "NOTFORGE_ADAPTER_TEST".to_string(),
            })
            .unwrap();

        assert_eq!(value, "resolved-value");
        unsafe { std::env::remove_var("NOTFORGE_ADAPTER_TEST") };
    }

    #[test]
    fn adapter_errors_when_env_secret_missing() {
        let resolver = env_only_resolver();
        let adapter = NotsecretsResolverAdapter::new(&resolver);

        let result = adapter.resolve(&ForgeSecretRef::Env {
            key: "NOTFORGE_ADAPTER_TEST_MISSING_99999".to_string(),
        });

        assert!(result.is_err());
    }

    #[test]
    fn adapter_rejects_op_secret_refs() {
        let resolver = env_only_resolver();
        let adapter = NotsecretsResolverAdapter::new(&resolver);

        let result = adapter.resolve(&ForgeSecretRef::Op {
            uri: "op://Personal/gitea/token".to_string(),
        });

        assert!(result.is_err());
    }
}
