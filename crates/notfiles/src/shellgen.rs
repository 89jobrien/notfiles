//! Generates native shell config files from a package's canonical shell IR
//! (`notshell::ShellConfig`), driven by `[packages.<name>].shell` in
//! notfiles.toml. Runs as a pre-pass before the normal symlink/copy pass so
//! generated files are treated exactly like hand-authored ones downstream.

use std::path::Path;

use notcore::{Config, NotfilesError};
use notshell::ShellConfig;

use crate::ports::FileStore;

/// Regenerate shell config files for every package that declares a `shell` block.
/// Returns the list of generated file paths (relative to `dotfiles_dir`).
pub fn generate_all(
    dotfiles_dir: &Path,
    config: &Config,
    fs: &dyn FileStore,
) -> Result<Vec<String>, NotfilesError> {
    let mut generated = Vec::new();
    for (package, pkg_config) in &config.packages {
        let Some(shell_config) = &pkg_config.shell else {
            continue;
        };
        let pkg_dir = dotfiles_dir.join(package);
        let source_path = pkg_dir.join(&shell_config.source);
        let ir_toml = fs.read_to_string(&source_path).map_err(|e| {
            NotfilesError::Shell(format!(
                "reading shell IR {} for package `{package}`: {e}",
                source_path.display()
            ))
        })?;
        let ir = ShellConfig::from_toml_str(&ir_toml)
            .map_err(|e| NotfilesError::Shell(format!("package `{package}`: {e}")))?;

        for (shell_id, rel_target) in &shell_config.targets {
            let emitter = notshell::emitter_for(shell_id)
                .map_err(|e| NotfilesError::Shell(format!("package `{package}`: {e}")))?;
            let contents = emitter.emit(&ir);
            let target_path = pkg_dir.join(rel_target);
            if let Some(parent) = target_path.parent() {
                fs.create_dir_all(parent).map_err(|e| {
                    NotfilesError::Shell(format!(
                        "creating {} for package `{package}`: {e}",
                        parent.display()
                    ))
                })?;
            }
            fs.write(&target_path, contents.as_bytes()).map_err(|e| {
                NotfilesError::Shell(format!(
                    "writing {} for package `{package}`: {e}",
                    target_path.display()
                ))
            })?;
            generated.push(format!("{package}/{rel_target}"));
        }
    }
    Ok(generated)
}
