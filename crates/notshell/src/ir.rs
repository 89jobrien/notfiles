use serde::Deserialize;

/// A guard that gates whether an entry is emitted/active.
/// Maps to `command -q`/`command -v` in fish/zsh/bash and `(which x | is-not-empty)` in nu.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Guard {
    CommandExists(String),
    PathExists(String),
}

#[derive(Debug, Clone, Deserialize)]
pub struct Alias {
    pub name: String,
    pub command: String,
    /// If the guard fails, emit `else_command` under `name` instead of skipping it entirely.
    /// Covers cases like `ide` falling back to `nvim .` when `zed` isn't installed.
    #[serde(default)]
    pub guard: Option<Guard>,
    #[serde(default)]
    pub else_command: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub guard: Option<Guard>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathEntry {
    pub dir: String,
    #[serde(default)]
    pub prepend: bool,
    #[serde(default)]
    pub guard: Option<Guard>,
}

/// Canonical, shell-agnostic config. This is the single source of truth a
/// package author writes once; per-shell files are generated from it.
///
/// Only expresses the common 80%: env/PATH/alias with a simple existence
/// guard. Anything more complex (arbitrary wrapper logic, multi-branch
/// functions) stays a hand-written per-shell file — notshell fails loudly
/// rather than growing an ad-hoc scripting language to cover it.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ShellConfig {
    #[serde(default)]
    pub env: Vec<EnvVar>,
    #[serde(default)]
    pub path: Vec<PathEntry>,
    #[serde(default)]
    pub aliases: Vec<Alias>,
}

impl ShellConfig {
    pub fn from_toml_str(s: &str) -> Result<Self, crate::NotshellError> {
        toml::from_str(s).map_err(|e| crate::NotshellError::ParseFailed {
            path: "<in-memory>".to_string(),
            detail: e.to_string(),
        })
    }
}
