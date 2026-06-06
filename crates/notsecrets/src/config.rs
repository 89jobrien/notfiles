use crate::error::SecretsError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Env,
    Op,
    Dotenvx,
    Sops,
    Gsm,
    Nuenv,
    Direnv,
    Mise,
    Bitwarden,
    Vault,
    Dotenvy,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ProviderConfig {
    Env,
    Op { account: String },
    Dotenvx { env_file: PathBuf },
    Sops { file: PathBuf },
    Gsm { project: String },
    Nuenv,
    Direnv,
    Mise,
    Bitwarden { server_url: Option<String> },
    Vault { addr: String, mount: Option<String> },
    Dotenvy { path: PathBuf },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum SecretRef {
    Env,
    Op { uri: String },
    Dotenvx { key: Option<String> },
    Sops { key: Option<String> },
    Gsm { path: String },
    Nuenv { key: Option<String> },
    Direnv { key: Option<String> },
    Mise { key: Option<String> },
    Bitwarden { item: String, field: Option<String> },
    Vault { path: String, field: Option<String> },
    Dotenvy { key: Option<String> },
}

impl SecretRef {
    pub fn provider(&self) -> Provider {
        match self {
            Self::Env { .. } => Provider::Env,
            Self::Op { .. } => Provider::Op,
            Self::Dotenvx { .. } => Provider::Dotenvx,
            Self::Sops { .. } => Provider::Sops,
            Self::Gsm { .. } => Provider::Gsm,
            Self::Nuenv { .. } => Provider::Nuenv,
            Self::Direnv { .. } => Provider::Direnv,
            Self::Mise { .. } => Provider::Mise,
            Self::Bitwarden { .. } => Provider::Bitwarden,
            Self::Vault { .. } => Provider::Vault,
            Self::Dotenvy { .. } => Provider::Dotenvy,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SecretsConfig {
    pub providers: Vec<Provider>,
    #[serde(default)]
    pub provider: HashMap<Provider, ProviderConfig>,
    #[serde(default)]
    pub secrets: HashMap<String, SecretRef>,
}

pub fn load_config(path: &Path) -> Result<SecretsConfig, SecretsError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| SecretsError::Config(format!("cannot read {}: {e}", path.display())))?;
    toml::from_str(&content).map_err(|e| SecretsError::Config(format!("parse error: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_deserializes_from_string() {
        let p: Provider = toml::Value::String("op".to_string()).try_into().unwrap();
        assert_eq!(p, Provider::Op);
    }

    #[test]
    fn secrets_config_parses_minimal() {
        let toml_str = r#"
            providers = ["env", "op"]
        "#;
        let cfg: SecretsConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.providers, vec![Provider::Env, Provider::Op]);
        assert!(cfg.provider.is_empty());
        assert!(cfg.secrets.is_empty());
    }

    #[test]
    fn secrets_config_parses_full() {
        let toml_str = r#"
            providers = ["env", "op", "dotenvx", "sops", "gsm"]

            [provider.op]
            type = "op"
            account = "my.1password.com"

            [provider.dotenvx]
            type = "dotenvx"
            env_file = "~/dev/.env"

            [provider.sops]
            type = "sops"
            file = "secrets/bootstrap.sops.env"

            [provider.gsm]
            type = "gsm"
            project = "my-gcp-project"

            [secrets]
            DATABASE_URL = { source = "op", uri = "op://Personal/db/url" }
            ANTHROPIC_API_KEY = { source = "dotenvx" }
            GCP_TOKEN = { source = "gsm", path = "projects/123/secrets/gcp-token/versions/latest" }
        "#;
        let cfg: SecretsConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.providers.len(), 5);
        assert_eq!(cfg.provider.len(), 4);
        assert_eq!(cfg.secrets.len(), 3);
        assert_eq!(cfg.secrets["DATABASE_URL"].provider(), Provider::Op);
    }

    #[test]
    fn secret_ref_provider_derivation() {
        let r = SecretRef::Gsm {
            path: "projects/123/secrets/tok/versions/latest".to_string(),
        };
        assert_eq!(r.provider(), Provider::Gsm);
    }

    #[test]
    fn unknown_provider_in_toml_errors() {
        let toml_str = r#"providers = ["env", "redis"]"#;
        let result: Result<SecretsConfig, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn load_config_reads_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("notsecrets.toml");
        std::fs::write(&path, "providers = [\"env\"]\n").unwrap();
        let cfg = load_config(&path).unwrap();
        assert_eq!(cfg.providers, vec![Provider::Env]);
    }

    #[test]
    fn load_config_missing_file_errors() {
        let result = load_config(std::path::Path::new("/nonexistent/notsecrets.toml"));
        assert!(result.is_err());
    }
}
