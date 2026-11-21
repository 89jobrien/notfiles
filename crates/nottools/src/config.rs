use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use notcore::expand_tilde;
use serde::Deserialize;

/// The `[doctor]` table of `notfiles.toml`.
///
/// Relative paths resolve against the dotfiles directory; `~` expands to the home directory.
/// Entries in `alias_files` may be files or directories (one level deep).
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct DoctorConfig {
    /// mise config declaring tools under `[tools]`.
    pub mise: Option<String>,
    #[serde(default)]
    pub brewfiles: Vec<String>,
    #[serde(default)]
    pub alias_files: Vec<String>,
}

#[derive(Deserialize)]
struct Root {
    doctor: Option<DoctorConfig>,
}

impl DoctorConfig {
    /// Parse the `[doctor]` table from the text of `notfiles.toml`.
    /// Returns `None` when the table is absent.
    pub fn parse(toml_text: &str) -> Result<Option<Self>> {
        let root: Root = toml::from_str(toml_text).context("invalid notfiles.toml")?;
        Ok(root.doctor)
    }

    pub fn mise_path(&self, dotfiles_dir: &Path) -> Result<Option<PathBuf>> {
        self.mise
            .as_deref()
            .map(|p| resolve(dotfiles_dir, p))
            .transpose()
    }

    pub fn brewfile_paths(&self, dotfiles_dir: &Path) -> Result<Vec<PathBuf>> {
        self.brewfiles
            .iter()
            .map(|p| resolve(dotfiles_dir, p))
            .collect()
    }

    pub fn alias_paths(&self, dotfiles_dir: &Path) -> Result<Vec<PathBuf>> {
        self.alias_files
            .iter()
            .map(|p| resolve(dotfiles_dir, p))
            .collect()
    }
}

fn resolve(dotfiles_dir: &Path, raw: &str) -> Result<PathBuf> {
    if raw.starts_with('~') {
        return expand_tilde(raw).with_context(|| format!("cannot expand {raw}"));
    }
    let p = Path::new(raw);
    Ok(if p.is_absolute() {
        p.to_path_buf()
    } else {
        dotfiles_dir.join(p)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_doctor_table() {
        let c = DoctorConfig::parse(
            "[defaults]\ntarget = \"~\"\n[doctor]\nmise = \"mise/config.toml\"\nbrewfiles = [\"Brewfile\"]\nalias_files = [\"~/.zshrc\"]\n",
        )
        .expect("parse should succeed")
        .expect("the value should be present");
        assert_eq!(c.mise.as_deref(), Some("mise/config.toml"));
        assert_eq!(c.brewfiles, ["Brewfile"]);
        assert_eq!(c.alias_files, ["~/.zshrc"]);
    }

    #[test]
    fn absent_table_is_none() {
        assert_eq!(
            DoctorConfig::parse("[defaults]\ntarget = \"~\"\n").expect("parse should succeed"),
            None
        );
    }

    #[test]
    fn relative_paths_join_dotfiles_dir() {
        let c = DoctorConfig {
            mise: Some("mise/c.toml".into()),
            ..Default::default()
        };
        let got = c
            .mise_path(Path::new("/d"))
            .expect("mise_path should succeed")
            .expect("the value should be present");
        assert_eq!(got, Path::new("/d/mise/c.toml"));
    }
}
