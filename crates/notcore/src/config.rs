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
    /// If set, only these package names are discovered.
    /// Mutually exclusive with `exclude`.
    #[serde(default)]
    pub include: Vec<String>,
    /// If set, these package names are skipped during discovery.
    /// Mutually exclusive with `include`.
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            target: default_target(),
            ignore: default_ignore(),
            include: Vec::new(),
            exclude: Vec::new(),
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
    /// If non-empty, this package is only linked on these platforms.
    /// Valid values: "macos", "linux".
    #[serde(default)]
    pub platforms: Vec<String>,
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
        let content = std::fs::read_to_string(&config_path).map_err(|e| {
            NotfilesError::Config(format!("reading {}: {e}", config_path.display()))
        })?;
        let config: Config = toml::from_str(&content).map_err(|e| {
            NotfilesError::Config(format!("parsing {}: {e}", config_path.display()))
        })?;
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

    /// Returns the include list if non-empty, or None (meaning all).
    pub fn included_packages(&self) -> Option<Vec<&str>> {
        if self.defaults.include.is_empty() {
            None
        } else {
            let mut v: Vec<&str> = self.defaults.include.iter().map(|s| s.as_str()).collect();
            v.sort();
            Some(v)
        }
    }

    /// Returns true if this package name appears in the exclude list.
    pub fn is_package_excluded(&self, name: &str) -> bool {
        self.defaults.exclude.iter().any(|e| e == name)
    }

    /// Validate config invariants. Returns Err if include and exclude
    /// are both non-empty.
    pub fn validate(&self) -> Result<(), NotfilesError> {
        if !self.defaults.include.is_empty() && !self.defaults.exclude.is_empty() {
            return Err(NotfilesError::Validation(
                "include and exclude are mutually exclusive".into(),
            ));
        }
        Ok(())
    }

    /// Returns true if the package should be linked on the current OS.
    /// Empty platforms list means "all platforms".
    pub fn is_package_for_current_platform(&self, package: &str) -> bool {
        let platforms = match self.packages.get(package) {
            Some(pkg) if !pkg.platforms.is_empty() => &pkg.platforms,
            _ => return true,
        };
        let current = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else {
            return false;
        };
        platforms.iter().any(|p| p == current)
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

/// Suggest the closest package name if the user made a typo.
/// Returns None if no close match (distance > 2).
pub fn suggest_package<'a>(name: &str, available: &[&'a str]) -> Option<&'a str> {
    available
        .iter()
        .filter_map(|candidate| {
            let dist = strsim::damerau_levenshtein(name, candidate);
            if dist <= 2 {
                Some((*candidate, dist))
            } else {
                None
            }
        })
        .min_by_key(|(_, d)| *d)
        .map(|(name, _)| name)
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
    fn test_include_filter_restricts_packages() {
        let toml_str = r#"
[defaults]
target = "~"
ignore = [".git"]
include = ["git", "zsh", "nushell"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.included_packages(),
            Some(vec!["git", "nushell", "zsh"]),
        );
    }

    #[test]
    fn test_exclude_filter_blocks_packages() {
        let toml_str = r#"
[defaults]
target = "~"
ignore = [".git"]
exclude = ["scripts", "docs", "tests"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.is_package_excluded("scripts"));
        assert!(!config.is_package_excluded("zsh"));
    }

    #[test]
    fn test_include_and_exclude_mutual_exclusion() {
        let toml_str = r#"
[defaults]
target = "~"
include = ["git"]
exclude = ["scripts"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_no_include_no_exclude_returns_none() {
        let config = Config::default();
        assert_eq!(config.included_packages(), None);
        assert!(!config.is_package_excluded("anything"));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_platform_filter_linux_only() {
        let toml_str = r#"
[defaults]
target = "~"

[packages.nixos]
platforms = ["linux"]

[packages.git]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        // On macOS this should be false, on Linux true
        if cfg!(target_os = "macos") {
            assert!(!config.is_package_for_current_platform("nixos"));
        } else if cfg!(target_os = "linux") {
            assert!(config.is_package_for_current_platform("nixos"));
        }
        // git has no platform filter — always matches
        assert!(config.is_package_for_current_platform("git"));
        // unknown package — no config entry, always matches
        assert!(config.is_package_for_current_platform("zsh"));
    }

    #[test]
    fn test_platform_filter_macos_only() {
        let toml_str = r#"
[packages.vscode]
platforms = ["macos"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        if cfg!(target_os = "macos") {
            assert!(config.is_package_for_current_platform("vscode"));
        } else {
            assert!(!config.is_package_for_current_platform("vscode"));
        }
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
