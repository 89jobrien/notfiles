use serde::{Deserialize, Deserializer, Serialize};
use std::path::Path;

use crate::error::NotforgeError;

/// Top-level forge configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgeConfig {
    /// Gitea backend configuration.
    pub gitea: GiteaConfig,
    /// Repositories managed by this forge configuration.
    #[serde(default)]
    pub repositories: Vec<RepoSpec>,
}

/// Gitea backend configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GiteaConfig {
    /// Base HTTP URL for the Gitea API.
    pub base_url: String,
    /// Default owner namespace for repositories.
    pub owner: String,
    /// Hostname used in generated SSH clone URLs.
    pub ssh_host: String,
    /// SSH port used in generated SSH clone URLs.
    pub ssh_port: u16,
    /// Backend mode used to verify or start Gitea.
    pub mode: GiteaMode,
    /// Authentication configuration for Gitea API calls.
    #[serde(default)]
    pub auth: ForgeAuthConfig,
}

/// Supported Gitea backend modes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum GiteaMode {
    /// Run Gitea directly as a local process.
    LocalProcess(LocalProcessConfig),
    /// Run Gitea via a local container runtime.
    LocalContainer(LocalContainerConfig),
    /// Verify or start Gitea as a systemd service on a VM.
    VmSystemd(VmSystemdConfig),
    /// Use an already-running Gitea instance.
    Existing,
}

impl<'de> Deserialize<'de> for GiteaMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawMode {
            Name(String),
            Tagged {
                #[serde(rename = "type")]
                kind: String,
                #[serde(default)]
                config: Option<toml::Value>,
            },
        }

        let raw = RawMode::deserialize(deserializer)?;
        match raw {
            RawMode::Name(name) if name == "existing" => Ok(Self::Existing),
            RawMode::Name(name) => Err(serde::de::Error::custom(format!(
                "unknown gitea mode '{name}'"
            ))),
            RawMode::Tagged { kind, config } => match kind.as_str() {
                "existing" => Ok(Self::Existing),
                "local-process" => Ok(Self::LocalProcess(
                    config
                        .ok_or_else(|| serde::de::Error::custom("local-process requires config"))?
                        .try_into()
                        .map_err(serde::de::Error::custom)?,
                )),
                "local-container" => Ok(Self::LocalContainer(
                    config
                        .ok_or_else(|| serde::de::Error::custom("local-container requires config"))?
                        .try_into()
                        .map_err(serde::de::Error::custom)?,
                )),
                "vm-systemd" => Ok(Self::VmSystemd(
                    config
                        .ok_or_else(|| serde::de::Error::custom("vm-systemd requires config"))?
                        .try_into()
                        .map_err(serde::de::Error::custom)?,
                )),
                other => Err(serde::de::Error::custom(format!(
                    "unknown gitea mode '{other}'"
                ))),
            },
        }
    }
}

/// Local process runner configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalProcessConfig {
    /// Path or command name for the Gitea binary.
    pub binary: String,
    /// Working directory for the Gitea process.
    pub work_dir: String,
    /// Gitea app.ini path.
    pub config_path: String,
}

/// Local container runner configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalContainerConfig {
    /// Container runtime executable, such as docker or podman.
    pub runtime: String,
    /// Container name to verify or start.
    pub container_name: String,
    /// Container image to run when missing.
    pub image: String,
    /// Host data directory mounted into the container.
    pub data_dir: String,
}

/// Remote VM systemd runner configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmSystemdConfig {
    /// SSH user for the VM.
    pub ssh_user: String,
    /// SSH host for the VM.
    pub ssh_host: String,
    /// systemd service name for Gitea.
    pub service_name: String,
}

/// Authentication configuration for Gitea API calls.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ForgeAuthConfig {
    /// Token secret reference, preferred when available.
    pub token: Option<ForgeSecretRef>,
    /// Username secret reference for basic auth fallback.
    pub username: Option<ForgeSecretRef>,
    /// Password secret reference for basic auth fallback.
    pub password: Option<ForgeSecretRef>,
}

/// Secret reference used by notforge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum ForgeSecretRef {
    /// Resolve from an environment variable.
    Env { key: String },
    /// Resolve from a 1Password URI.
    Op { uri: String },
}

/// Repository specification managed by a forge config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSpec {
    /// Repository owner namespace.
    pub owner: String,
    /// Repository name.
    pub name: String,
    /// Whether the repository should be private.
    pub private: bool,
    /// Optional repository description.
    pub description: Option<String>,
    /// Local git remote name.
    pub remote_name: String,
}

/// Load a forge configuration from TOML.
pub fn load_config(path: &Path) -> Result<ForgeConfig, NotforgeError> {
    notcore::config::load_toml_file(
        path,
        |path, err| NotforgeError::Config(format!("reading {}: {err}", path.display())),
        |path, err| NotforgeError::Config(format!("parsing {}: {err}", path.display())),
    )
}
