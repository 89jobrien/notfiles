use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config::{Config, Method};
use crate::error::NotfilesError;
use crate::package::collect_files;
use crate::paths::expand_tilde;

const STATE_FILE: &str = ".notfiles-state.toml";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateEntry {
    pub package: String,
    pub source: String,
    pub target: String,
    pub method: Method,
    pub linked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    #[serde(default)]
    pub entries: Vec<StateEntry>,
}

impl State {
    pub fn load(dotfiles_dir: &Path) -> Result<Self, NotfilesError> {
        let path = dotfiles_dir.join(STATE_FILE);
        if !path.exists() {
            return Ok(State::default());
        }
        let content = fs::read_to_string(&path)
            .map_err(|e| NotfilesError::State(format!("reading state: {e}")))?;
        let state: State = toml::from_str(&content)
            .map_err(|e| NotfilesError::State(format!("parsing state: {e}")))?;
        Ok(state)
    }

    pub fn save(&self, dotfiles_dir: &Path) -> Result<(), NotfilesError> {
        let path = dotfiles_dir.join(STATE_FILE);
        let content = toml::to_string_pretty(self)
            .map_err(|e| NotfilesError::State(format!("serializing state: {e}")))?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn entries_for_package(&self, package: &str) -> Vec<&StateEntry> {
        self.entries.iter().filter(|e| e.package == package).collect()
    }

    pub fn remove_package(&mut self, package: &str) {
        self.entries.retain(|e| e.package != package);
    }

    pub fn add_entry(&mut self, entry: StateEntry) {
        // Remove existing entry for same source+target, then add new
        self.entries.retain(|e| !(e.source == entry.source && e.target == entry.target));
        self.entries.push(entry);
    }
}

pub struct LinkOptions {
    pub force: bool,
    pub no_backup: bool,
    pub dry_run: bool,
    pub verbose: bool,
}

pub fn link_package(
    dotfiles_dir: &Path,
    config: &Config,
    state: &mut State,
    package: &str,
    opts: &LinkOptions,
) -> Result<(), NotfilesError> {
    let package_dir = dotfiles_dir.join(package);
    let method = config.method_for(package);
    let target_base = expand_tilde(config.target_for(package))?;
    let files = collect_files(&package_dir, config, package)?;

    if files.is_empty() {
        if opts.verbose {
            println!("  {package}: no files to link");
        }
        return Ok(());
    }

    for relative in &files {
        let source = package_dir.join(relative);
        let target = target_base.join(relative);
        let source_display = format!("{package}/{}", relative.display());

        // Check if already correctly linked
        if is_already_linked(&source, &target, method) {
            if opts.verbose {
                println!("  \x1b[90mskip\x1b[0m {source_display} (already linked)");
            }
            continue;
        }

        // Conflict detection
        if target.exists() || target.symlink_metadata().is_ok() {
            if !opts.force {
                return Err(NotfilesError::Conflict {
                    path: target.clone(),
                    reason: format!(
                        "already exists (use --force to overwrite); source: {source_display}"
                    ),
                });
            }
            // Force mode: backup then remove
            if !opts.no_backup {
                let backup = backup_path(&target);
                if opts.dry_run {
                    println!("  \x1b[33mwould backup\x1b[0m {} -> {}", target.display(), backup.display());
                } else {
                    if opts.verbose {
                        println!("  \x1b[33mbackup\x1b[0m {} -> {}", target.display(), backup.display());
                    }
                    fs::rename(&target, &backup)?;
                }
            } else if !opts.dry_run {
                if target.is_dir() {
                    fs::remove_dir_all(&target)?;
                } else {
                    fs::remove_file(&target)?;
                }
            }
        }

        // Create parent directories
        if let Some(parent) = target.parent() {
            if !parent.exists() {
                if opts.dry_run {
                    if opts.verbose {
                        println!("  \x1b[90mwould create dir\x1b[0m {}", parent.display());
                    }
                } else {
                    fs::create_dir_all(parent)?;
                }
            }
        }

        // Create link or copy
        let action_word = match method {
            Method::Symlink => "link",
            Method::Copy => "copy",
        };

        if opts.dry_run {
            println!("  \x1b[36mwould {action_word}\x1b[0m {source_display} -> {}", target.display());
        } else {
            match method {
                Method::Symlink => {
                    #[cfg(unix)]
                    std::os::unix::fs::symlink(&source, &target)?;
                    #[cfg(not(unix))]
                    fs::copy(&source, &target)?;
                }
                Method::Copy => {
                    fs::copy(&source, &target)?;
                }
            }
            if opts.verbose {
                println!("  \x1b[32m{action_word}\x1b[0m {source_display} -> {}", target.display());
            }

            state.add_entry(StateEntry {
                package: package.to_string(),
                source: source.to_string_lossy().to_string(),
                target: target.to_string_lossy().to_string(),
                method,
                linked_at: Utc::now().to_rfc3339(),
            });
        }
    }

    Ok(())
}

pub fn unlink_package(
    _dotfiles_dir: &Path,
    state: &mut State,
    package: &str,
    opts: &LinkOptions,
) -> Result<(), NotfilesError> {
    let entries: Vec<StateEntry> = state.entries_for_package(package).into_iter().cloned().collect();

    if entries.is_empty() {
        if opts.verbose {
            println!("  {package}: nothing to unlink");
        }
        return Ok(());
    }

    for entry in &entries {
        let target = PathBuf::from(&entry.target);

        if !target.exists() && target.symlink_metadata().is_err() {
            if opts.verbose {
                println!("  \x1b[90mskip\x1b[0m {} (already gone)", target.display());
            }
            continue;
        }

        match entry.method {
            Method::Symlink => {
                // Verify it's a symlink pointing to our source
                if let Ok(link_target) = fs::read_link(&target) {
                    let source = PathBuf::from(&entry.source);
                    if link_target != source {
                        if opts.verbose {
                            println!(
                                "  \x1b[33mskip\x1b[0m {} (symlink points elsewhere)",
                                target.display()
                            );
                        }
                        continue;
                    }
                } else {
                    if opts.verbose {
                        println!("  \x1b[33mskip\x1b[0m {} (not a symlink)", target.display());
                    }
                    continue;
                }
            }
            Method::Copy => {
                // For copies, trust the state file
            }
        }

        if opts.dry_run {
            println!("  \x1b[36mwould remove\x1b[0m {}", target.display());
        } else {
            if target.is_dir() {
                fs::remove_dir_all(&target)?;
            } else {
                fs::remove_file(&target)?;
            }
            if opts.verbose {
                println!("  \x1b[31mremove\x1b[0m {}", target.display());
            }

            // Clean up empty parent dirs
            cleanup_empty_parents(&target);
        }
    }

    if !opts.dry_run {
        state.remove_package(package);
    }

    Ok(())
}

fn is_already_linked(source: &Path, target: &Path, method: Method) -> bool {
    match method {
        Method::Symlink => {
            if let Ok(link_target) = fs::read_link(target) {
                link_target == source
            } else {
                false
            }
        }
        Method::Copy => false, // Always re-copy
    }
}

fn backup_path(path: &Path) -> PathBuf {
    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let name = path.to_string_lossy();
    PathBuf::from(format!("{name}.notfiles-backup-{timestamp}"))
}

fn cleanup_empty_parents(path: &Path) {
    let mut dir = path.parent();
    while let Some(parent) = dir {
        // Stop at home dir or root
        if Some(parent.to_path_buf()) == dirs::home_dir() || parent == Path::new("/") {
            break;
        }
        if fs::read_dir(parent).map(|mut d| d.next().is_none()).unwrap_or(false) {
            let _ = fs::remove_dir(parent);
            dir = parent.parent();
        } else {
            break;
        }
    }
}
