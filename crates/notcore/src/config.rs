use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::NotfilesError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(default)]
    pub packages: HashMap<String, PackageConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defaults {
    #[serde(default = "default_target")]
    pub target: String,
    #[serde(default = "default_ignore")]
    pub ignore: Vec<String>,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            target: default_target(),
            ignore: default_ignore(),
        }
    }
}

fn default_target() -> String {
    "~".to_string()
}

fn default_ignore() -> Vec<String> {
    vec![
        ".git".to_string(),
        ".DS_Store".to_string(),
        "README.md".to_string(),
        "LICENSE".to_string(),
        "notfiles.toml".to_string(),
        ".notfiles-state.toml".to_string(),
        ".nothooks-state.toml".to_string(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackageConfig {
    #[serde(default)]
    pub method: Option<Method>,
    pub target: Option<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    #[default]
    Symlink,
    Copy,
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Method::Symlink => write!(f, "symlink"),
            Method::Copy => write!(f, "copy"),
        }
    }
}

impl Config {
    pub fn load(dotfiles_dir: &Path) -> Result<Self, NotfilesError> {
        let config_path = dotfiles_dir.join("notfiles.toml");
        if !config_path.exists() {
            return Ok(Config::default());
        }
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| NotfilesError::Config(format!("reading {}: {e}", config_path.display())))?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| NotfilesError::Config(format!("parsing {}: {e}", config_path.display())))?;
        Ok(config)
    }

    pub fn method_for(&self, package: &str) -> Method {
        self.packages
            .get(package)
            .and_then(|p| p.method)
            .unwrap_or_default()
    }

    pub fn target_for(&self, package: &str) -> &str {
        self.packages
            .get(package)
            .and_then(|p| p.target.as_deref())
            .unwrap_or(&self.defaults.target)
    }

    pub fn ignore_patterns_for(&self, package: &str) -> Vec<&str> {
        let mut patterns: Vec<&str> = self.defaults.ignore.iter().map(|s| s.as_str()).collect();
        if let Some(pkg) = self.packages.get(package) {
            for p in &pkg.ignore {
                patterns.push(p.as_str());
            }
        }
        patterns
    }
}

pub fn starter_toml() -> &'static str {
    r#"[defaults]
target = "~"
ignore = [".git", ".DS_Store", "README.md", "LICENSE", "notfiles.toml", ".notfiles-state.toml", ".nothooks-state.toml"]

# [packages.ssh]
# method = "copy"
# ignore = ["known_hosts"]
#
# [packages.scripts]
# target = "~/bin"
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.defaults.target, "~");
        assert!(config.defaults.ignore.contains(&".git".to_string()));
        assert_eq!(config.method_for("anything"), Method::Symlink);
        assert_eq!(config.target_for("anything"), "~");
    }

    #[test]
    fn test_parse_config() {
        let toml_str = r#"
[defaults]
target = "~"
ignore = [".git"]

[packages.ssh]
method = "copy"
ignore = ["known_hosts"]

[packages.scripts]
target = "~/bin"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.method_for("ssh"), Method::Copy);
        assert_eq!(config.method_for("scripts"), Method::Symlink);
        assert_eq!(config.target_for("scripts"), "~/bin");
        assert_eq!(config.target_for("ssh"), "~");

        let ssh_ignores = config.ignore_patterns_for("ssh");
        assert!(ssh_ignores.contains(&".git"));
        assert!(ssh_ignores.contains(&"known_hosts"));
    }
}
