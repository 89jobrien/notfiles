//! Builds provider chains and resolves configured secrets by priority.

use crate::config::{Provider, ProviderConfig, SecretsConfig};
use crate::error::SecretsError;
use crate::ports::{EnumerableSecretSource, SecretSource};
use crate::sources::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn expand_provider_path(path: &Path) -> Result<PathBuf, SecretsError> {
    let value = path.to_str().ok_or_else(|| {
        SecretsError::Config(format!(
            "provider path is not valid UTF-8: {}",
            path.display()
        ))
    })?;
    notcore::expand_tilde(value)
        .map_err(|error| SecretsError::Config(format!("cannot expand provider path: {error}")))
}

fn expand_provider_paths(config: &mut SecretsConfig) -> Result<(), SecretsError> {
    for provider in config.provider.values_mut() {
        match provider {
            ProviderConfig::Dotenvx { env_file } => {
                *env_file = expand_provider_path(env_file)?;
            }
            ProviderConfig::Sops { file } => {
                *file = expand_provider_path(file)?;
            }
            ProviderConfig::Dotenvy { path } => {
                *path = expand_provider_path(path)?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub struct SecretResolver {
    config: SecretsConfig,
    sources: Vec<Box<dyn SecretSource>>,
    enumerable: Vec<Box<dyn EnumerableSecretSource>>,
}

impl SecretResolver {
    /// Validates configuration and constructs its provider chain.
    pub fn from_config(mut config: SecretsConfig) -> Result<Self, SecretsError> {
        expand_provider_paths(&mut config)?;

        // Validate: every binding references a provider in the chain
        for (key, binding) in &config.secrets {
            if !config.providers.contains(&binding.provider()) {
                return Err(SecretsError::Config(format!(
                    "secret '{key}' references provider '{:?}' not in providers list",
                    binding.provider()
                )));
            }
        }

        let mut sources: Vec<Box<dyn SecretSource>> = Vec::new();
        let mut enumerable: Vec<Box<dyn EnumerableSecretSource>> = Vec::new();

        for provider in &config.providers {
            let provider_config = config.provider.get(provider);
            match provider {
                Provider::Env => {
                    enumerable.push(Box::new(EnvSource));
                    sources.push(Box::new(EnvSource));
                }
                Provider::Op => {
                    let account = match provider_config {
                        Some(ProviderConfig::Op { account }) => account.clone(),
                        _ => {
                            return Err(SecretsError::Config(
                                "op provider requires [provider.op] with account".to_string(),
                            ));
                        }
                    };
                    sources.push(Box::new(OpSource::new(account)));
                }
                Provider::Dotenvx => {
                    let env_file = match provider_config {
                        Some(ProviderConfig::Dotenvx { env_file }) => env_file.clone(),
                        _ => {
                            return Err(SecretsError::Config(
                                "dotenvx provider requires [provider.dotenvx] with env_file"
                                    .to_string(),
                            ));
                        }
                    };
                    enumerable.push(Box::new(DotenvxSource::new(env_file.clone())));
                    sources.push(Box::new(DotenvxSource::new(env_file)));
                }
                Provider::Sops => {
                    let file = match provider_config {
                        Some(ProviderConfig::Sops { file }) => file.clone(),
                        _ => {
                            return Err(SecretsError::Config(
                                "sops provider requires [provider.sops] with file".to_string(),
                            ));
                        }
                    };
                    enumerable.push(Box::new(SopsSource::new(file.clone())));
                    sources.push(Box::new(SopsSource::new(file)));
                }
                Provider::Gsm => {
                    let project = match provider_config {
                        Some(ProviderConfig::Gsm { project }) => project.clone(),
                        _ => {
                            return Err(SecretsError::Config(
                                "gsm provider requires [provider.gsm] with project".to_string(),
                            ));
                        }
                    };
                    sources.push(Box::new(GsmSource::new(project)));
                }
                Provider::Nuenv => sources.push(Box::new(NuenvSource)),
                Provider::Direnv => sources.push(Box::new(DirenvSource)),
                Provider::Mise => sources.push(Box::new(MiseSource)),
                Provider::Bitwarden => {
                    let item_name = match provider_config {
                        Some(ProviderConfig::Bitwarden { .. }) => "default".to_string(),
                        _ => "default".to_string(),
                    };
                    sources.push(Box::new(BitwardenSource::new(item_name)));
                }
                Provider::Vault => sources.push(Box::new(VaultSource)),
                Provider::Dotenvy => sources.push(Box::new(DotenvySource)),
            }
        }

        Ok(Self {
            config,
            sources,
            enumerable,
        })
    }

    /// Resolves one secret through its binding or the provider chain.
    pub fn resolve(&self, key: &str) -> Result<Option<String>, SecretsError> {
        // 1. Explicit binding
        if let Some(secret_ref) = self.config.secrets.get(key) {
            let provider = secret_ref.provider();
            if let Some(source) = self.sources.iter().find(|s| s.provider() == provider) {
                return source.resolve_ref(key, secret_ref);
            }
        }

        // 2. Priority chain
        for source in &self.sources {
            if let Some(value) = source.resolve(key)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    /// Merges enumerable providers and overlays explicit bindings.
    pub fn resolve_all(&self) -> Result<HashMap<String, String>, SecretsError> {
        let mut map = HashMap::new();

        // 1. Merge enumerable sources (priority order, first wins)
        for source in &self.enumerable {
            let source_map = source.resolve_all()?;
            for (k, v) in source_map {
                map.entry(k).or_insert(v);
            }
        }

        // 2. Overlay explicit bindings (always win)
        for (key, secret_ref) in &self.config.secrets {
            let provider = secret_ref.provider();
            if let Some(source) = self.sources.iter().find(|s| s.provider() == provider)
                && let Some(value) = source.resolve_ref(key, secret_ref)?
            {
                map.insert(key.clone(), value);
            }
        }

        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Provider, SecretRef, SecretsConfig};

    fn config_with_env_only() -> SecretsConfig {
        SecretsConfig {
            providers: vec![Provider::Env],
            provider: HashMap::new(),
            secrets: HashMap::new(),
        }
    }

    #[test]
    fn resolver_from_config_env_only() {
        let resolver = SecretResolver::from_config(config_with_env_only());
        assert!(resolver.is_ok());
    }

    #[test]
    fn resolver_resolve_env_var() {
        unsafe { std::env::set_var("NOTSECRETS_RESOLVER_TEST", "hello") };
        let resolver = SecretResolver::from_config(config_with_env_only()).unwrap();
        let val = resolver.resolve("NOTSECRETS_RESOLVER_TEST").unwrap();
        assert_eq!(val, Some("hello".to_string()));
        unsafe { std::env::remove_var("NOTSECRETS_RESOLVER_TEST") };
    }

    #[test]
    fn resolver_resolve_missing_returns_none() {
        let resolver = SecretResolver::from_config(config_with_env_only()).unwrap();
        let val = resolver.resolve("NOTSECRETS_NONEXISTENT_99999").unwrap();
        assert_eq!(val, None);
    }

    #[test]
    fn resolver_binding_overrides_chain() {
        unsafe { std::env::set_var("NOTSECRETS_BOUND_TEST", "from_env") };
        let mut config = config_with_env_only();
        config
            .secrets
            .insert("NOTSECRETS_BOUND_TEST".to_string(), SecretRef::Env);
        let resolver = SecretResolver::from_config(config).unwrap();
        let val = resolver.resolve("NOTSECRETS_BOUND_TEST").unwrap();
        assert_eq!(val, Some("from_env".to_string()));
        unsafe { std::env::remove_var("NOTSECRETS_BOUND_TEST") };
    }

    #[test]
    fn resolver_resolve_all_includes_env() {
        unsafe { std::env::set_var("NOTSECRETS_ALL_TEST", "present") };
        let resolver = SecretResolver::from_config(config_with_env_only()).unwrap();
        let map = resolver.resolve_all().unwrap();
        assert_eq!(
            map.get("NOTSECRETS_ALL_TEST").map(|s| s.as_str()),
            Some("present")
        );
        unsafe { std::env::remove_var("NOTSECRETS_ALL_TEST") };
    }

    #[test]
    fn resolver_binding_refs_unknown_provider_errors() {
        let config = SecretsConfig {
            providers: vec![Provider::Env],
            provider: HashMap::new(),
            secrets: HashMap::from([(
                "KEY".to_string(),
                SecretRef::Op {
                    uri: "op://x/y/z".to_string(),
                },
            )]),
        };
        let result = SecretResolver::from_config(config);
        assert!(result.is_err());
    }

    #[test]
    fn resolver_expands_tilde_in_filesystem_provider_paths() {
        let config = SecretsConfig {
            providers: vec![Provider::Dotenvx, Provider::Sops, Provider::Dotenvy],
            provider: HashMap::from([
                (
                    Provider::Dotenvx,
                    ProviderConfig::Dotenvx {
                        env_file: "~/.config/secrets.env".into(),
                    },
                ),
                (
                    Provider::Sops,
                    ProviderConfig::Sops {
                        file: "~/.config/secrets.sops.env".into(),
                    },
                ),
                (
                    Provider::Dotenvy,
                    ProviderConfig::Dotenvy {
                        path: "~/.config/secrets.dotenv".into(),
                    },
                ),
            ]),
            secrets: HashMap::new(),
        };

        let resolver = SecretResolver::from_config(config).unwrap();
        let home = notcore::expand_tilde("~").unwrap();

        assert!(matches!(
            resolver.config.provider.get(&Provider::Dotenvx),
            Some(ProviderConfig::Dotenvx { env_file })
                if env_file == &home.join(".config/secrets.env")
        ));
        assert!(matches!(
            resolver.config.provider.get(&Provider::Sops),
            Some(ProviderConfig::Sops { file })
                if file == &home.join(".config/secrets.sops.env")
        ));
        assert!(matches!(
            resolver.config.provider.get(&Provider::Dotenvy),
            Some(ProviderConfig::Dotenvy { path })
                if path == &home.join(".config/secrets.dotenv")
        ));
    }
}
