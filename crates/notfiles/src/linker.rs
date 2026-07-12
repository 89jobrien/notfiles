use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::package::collect_files_with_store;
use crate::ports::FileStore;
use notcore::reporter::{LinkEvent, Reporter};
use notcore::{Config, Method, NotfilesError, expand_tilde};

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
    pub fn load(dotfiles_dir: &Path, fs: &dyn FileStore) -> Result<Self, NotfilesError> {
        let path = dotfiles_dir.join(STATE_FILE);
        if !fs.exists(&path) {
            return Ok(State::default());
        }
        let content = fs
            .read_to_string(&path)
            .map_err(|e| NotfilesError::State(format!("reading state: {e}")))?;
        let state: State = toml::from_str(&content)
            .map_err(|e| NotfilesError::State(format!("parsing state: {e}")))?;
        Ok(state)
    }

    pub fn save(&self, dotfiles_dir: &Path, fs: &dyn FileStore) -> Result<(), NotfilesError> {
        let path = dotfiles_dir.join(STATE_FILE);
        let tmp_path = dotfiles_dir.join(format!("{STATE_FILE}.tmp"));
        let content = toml::to_string_pretty(self)
            .map_err(|e| NotfilesError::State(format!("serializing state: {e}")))?;
        fs.write(&tmp_path, content.as_bytes())
            .map_err(|e| NotfilesError::State(format!("writing temp state: {e}")))?;
        fs.rename(&tmp_path, &path)
            .map_err(|e| NotfilesError::State(format!("renaming temp state: {e}")))?;
        Ok(())
    }

    pub fn entries_for_package(&self, package: &str) -> Vec<&StateEntry> {
        self.entries
            .iter()
            .filter(|e| e.package == package)
            .collect()
    }

    pub fn remove_package(&mut self, package: &str) {
        self.entries.retain(|e| e.package != package);
    }

    pub fn remove_entry(&mut self, package: &str, source: &str, target: &str) {
        self.entries
            .retain(|e| !(e.package == package && e.source == source && e.target == target));
    }

    pub fn add_entry(&mut self, entry: StateEntry) {
        // Remove existing entry for same source+target, then add new
        self.entries
            .retain(|e| !(e.source == entry.source && e.target == entry.target));
        self.entries.push(entry);
    }
}

pub struct LinkOptions {
    pub force: bool,
    pub no_backup: bool,
    pub dry_run: bool,
    pub verbose: bool,
}

#[derive(Debug, Default)]
pub struct LinkResult {
    pub package: String,
    pub linked: usize,
    pub copied: usize,
    pub skipped: usize,
    pub backed_up: usize,
}

pub fn link_package(
    dotfiles_dir: &Path,
    config: &Config,
    state: &mut State,
    package: &str,
    opts: &LinkOptions,
    fs: &dyn FileStore,
    reporter: &dyn Reporter,
) -> Result<LinkResult, NotfilesError> {
    let package_dir = dotfiles_dir.join(package);
    let method = config.method_for(package);
    let target_base = expand_tilde(config.target_for(package))?;
    let files = collect_files_with_store(&package_dir, config, package, fs)?;
    let mut result = LinkResult {
        package: package.to_string(),
        ..Default::default()
    };

    if files.is_empty() {
        reporter.report(&LinkEvent::Skip {
            source: package,
            reason: "no files to link",
        });
        return Ok(result);
    }

    for relative in &files {
        let source = package_dir.join(relative);
        let target = target_base.join(relative);
        let source_display = format!("{package}/{}", relative.display());

        // Check if already correctly linked / copied
        if is_already_linked(&source, &target, method, fs) {
            reporter.report(&LinkEvent::Skip {
                source: &source_display,
                reason: "already linked",
            });
            result.skipped += 1;
            continue;
        }

        // Helper: save partial state then return an error.
        let save_and_return =
            |state: &mut State, e: NotfilesError| -> Result<LinkResult, NotfilesError> {
                if !opts.dry_run {
                    let _ = state.save(dotfiles_dir, fs);
                }
                Err(e)
            };

        // Conflict detection
        if fs.exists(&target) || fs.symlink_metadata(&target).is_ok() {
            if !opts.force {
                return save_and_return(
                    state,
                    NotfilesError::Conflict {
                        path: target.clone(),
                        reason: format!(
                            "already exists (use --force to overwrite); source: {source_display}"
                        ),
                    },
                );
            }
            // Force mode: backup then remove
            if !opts.no_backup {
                let backup = backup_path(&target);
                if opts.dry_run {
                    reporter.report(&LinkEvent::DryRun {
                        action: "backup",
                        source: &source_display,
                        target: &backup,
                    });
                } else {
                    reporter.report(&LinkEvent::Backup {
                        from: &target,
                        to: &backup,
                    });
                    if let Err(e) = fs.rename(&target, &backup) {
                        return save_and_return(state, e.into());
                    }
                    result.backed_up += 1;
                }
            } else if !opts.dry_run {
                let rm_result = if fs.is_dir(&target) {
                    fs.remove_dir_all(&target)
                } else {
                    fs.remove_file(&target)
                };
                if let Err(e) = rm_result {
                    return save_and_return(state, e.into());
                }
            }
        }

        // Create parent directories
        if let Some(parent) = target.parent().filter(|p| !fs.exists(p)) {
            if opts.dry_run {
                reporter.report(&LinkEvent::CreateDir { path: parent });
            } else if let Err(e) = fs.create_dir_all(parent) {
                return save_and_return(state, e.into());
            }
        }

        // Create link or copy
        let action_word = match method {
            Method::Symlink => "link",
            Method::Copy => "copy",
        };

        if opts.dry_run {
            reporter.report(&LinkEvent::DryRun {
                action: action_word,
                source: &source_display,
                target: &target,
            });
        } else {
            let io_result = match method {
                Method::Symlink => {
                    #[cfg(unix)]
                    {
                        fs.symlink(&source, &target).map(|_| 0u64)
                    }
                    #[cfg(not(unix))]
                    {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::Unsupported,
                            "symlink not supported on this platform",
                        ))
                    }
                }
                Method::Copy => {
                    let content = fs.read(&source)?;
                    fs.write(&target, &content).map(|_| content.len() as u64)
                }
            };
            if let Err(e) = io_result {
                return save_and_return(state, e.into());
            }
            match method {
                Method::Symlink => {
                    reporter.report(&LinkEvent::Link {
                        source: &source_display,
                        target: &target,
                    });
                    result.linked += 1;
                }
                Method::Copy => {
                    reporter.report(&LinkEvent::Copy {
                        source: &source_display,
                        target: &target,
                    });
                    result.copied += 1;
                }
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

    if !opts.dry_run {
        state.save(dotfiles_dir, fs)?;
    }
    Ok(result)
}

pub fn unlink_package(
    dotfiles_dir: &Path,
    state: &mut State,
    package: &str,
    opts: &LinkOptions,
    fs: &dyn FileStore,
    reporter: &dyn Reporter,
) -> Result<(), NotfilesError> {
    // Validate the package: it must either exist as a directory in dotfiles_dir
    // or have entries in state.  A name that satisfies neither is a user error.
    let package_dir = dotfiles_dir.join(package);
    let has_state_entries = !state.entries_for_package(package).is_empty();
    if !fs.is_dir(&package_dir) && !has_state_entries {
        return Err(NotfilesError::PackageNotFound {
            name: package.to_string(),
        });
    }

    let entries: Vec<StateEntry> = state
        .entries_for_package(package)
        .into_iter()
        .cloned()
        .collect();

    if entries.is_empty() {
        reporter.report(&LinkEvent::Skip {
            source: package,
            reason: "nothing to unlink",
        });
        return Ok(());
    }

    let mut removed_entries = Vec::new();

    for entry in &entries {
        let target = PathBuf::from(&entry.target);
        let source = PathBuf::from(&entry.source);

        if !fs.exists(&target) && fs.symlink_metadata(&target).is_err() {
            reporter.report(&LinkEvent::Skip {
                source: &target.to_string_lossy(),
                reason: "already gone",
            });
            if !opts.dry_run {
                removed_entries.push((entry.source.clone(), entry.target.clone()));
            }
            continue;
        }

        match entry.method {
            Method::Symlink => {
                // Verify it's a symlink pointing to our source
                if let Ok(link_target) = fs.read_link(&target) {
                    if link_target != source {
                        reporter.report(&LinkEvent::Skip {
                            source: &target.to_string_lossy(),
                            reason: "symlink points elsewhere",
                        });
                        continue;
                    }
                } else {
                    reporter.report(&LinkEvent::Skip {
                        source: &target.to_string_lossy(),
                        reason: "not a symlink",
                    });
                    continue;
                }
            }
            Method::Copy => match (fs.read(&source), fs.read(&target)) {
                (Ok(source_bytes), Ok(target_bytes)) if source_bytes == target_bytes => {}
                (Ok(_), Ok(_)) => {
                    reporter.report(&LinkEvent::Skip {
                        source: &target.to_string_lossy(),
                        reason: "copied file diverged from source",
                    });
                    continue;
                }
                (Err(_), _) => {
                    reporter.report(&LinkEvent::Skip {
                        source: &target.to_string_lossy(),
                        reason: "source missing for copied file",
                    });
                    continue;
                }
                (_, Err(_)) => {
                    reporter.report(&LinkEvent::Skip {
                        source: &target.to_string_lossy(),
                        reason: "cannot read copied target",
                    });
                    continue;
                }
            },
        }

        if opts.dry_run {
            reporter.report(&LinkEvent::DryRun {
                action: "remove",
                source: &target.to_string_lossy(),
                target: &target,
            });
        } else {
            if fs.is_dir(&target) {
                fs.remove_dir_all(&target)?;
            } else {
                fs.remove_file(&target)?;
            }
            reporter.report(&LinkEvent::Remove { target: &target });

            // Clean up empty parent dirs
            cleanup_empty_parents(&target, fs);
            removed_entries.push((entry.source.clone(), entry.target.clone()));
        }
    }

    if !opts.dry_run {
        for (source, target) in removed_entries {
            state.remove_entry(package, &source, &target);
        }
        state.save(dotfiles_dir, fs)?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn adopt_files(
    dotfiles_dir: &Path,
    config: &Config,
    state: &mut State,
    package: &str,
    files: &[String],
    opts: &LinkOptions,
    fs: &dyn FileStore,
    reporter: &dyn Reporter,
) -> Result<LinkResult, NotfilesError> {
    let package_dir = dotfiles_dir.join(package);
    let target_base = expand_tilde(config.target_for(package))?;
    let mut result = LinkResult {
        package: package.to_string(),
        ..Default::default()
    };

    if !fs.is_dir(&package_dir) {
        if opts.dry_run {
            reporter.report(&LinkEvent::CreateDir { path: &package_dir });
        } else {
            fs.create_dir_all(&package_dir)?;
        }
    }

    for file in files {
        let relative = PathBuf::from(file);
        let target = target_base.join(&relative);
        let source = package_dir.join(&relative);
        let display = format!("{package}/{}", relative.display());

        if !fs.exists(&target) {
            reporter.report(&LinkEvent::Skip {
                source: &display,
                reason: "target file does not exist",
            });
            result.skipped += 1;
            continue;
        }

        if fs.exists(&source) {
            reporter.report(&LinkEvent::Skip {
                source: &display,
                reason: "already exists in package",
            });
            result.skipped += 1;
            continue;
        }

        if opts.dry_run {
            reporter.report(&LinkEvent::DryRun {
                action: "adopt",
                source: &display,
                target: &target,
            });
            continue;
        }

        // Create parent dirs in package
        if let Some(parent) = source.parent().filter(|p| !fs.exists(p)) {
            fs.create_dir_all(parent)?;
        }

        // Move target -> package source
        fs.rename(&target, &source)?;

        // Create symlink target -> source
        #[cfg(unix)]
        {
            fs.symlink(&source, &target)?;
        }

        reporter.report(&LinkEvent::Link {
            source: &display,
            target: &target,
        });

        state.add_entry(StateEntry {
            package: package.to_string(),
            source: source.to_string_lossy().to_string(),
            target: target.to_string_lossy().to_string(),
            method: Method::Symlink,
            linked_at: Utc::now().to_rfc3339(),
        });
        result.linked += 1;
    }

    if !opts.dry_run {
        state.save(dotfiles_dir, fs)?;
    }
    Ok(result)
}

fn is_already_linked(source: &Path, target: &Path, method: Method, fs: &dyn FileStore) -> bool {
    match method {
        Method::Symlink => {
            if let Ok(link_target) = fs.read_link(target) {
                link_target == source
            } else {
                false
            }
        }
        Method::Copy => match (fs.read(source), fs.read(target)) {
            (Ok(source_bytes), Ok(target_bytes)) => source_bytes == target_bytes,
            _ => false,
        },
    }
}

fn backup_path(path: &Path) -> PathBuf {
    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let name = path.to_string_lossy();
    PathBuf::from(format!("{name}.notfiles-backup-{timestamp}"))
}

fn cleanup_empty_parents(path: &Path, fs: &dyn FileStore) {
    let mut dir = path.parent();
    while let Some(parent) = dir {
        // Stop at home dir or root
        if Some(parent.to_path_buf()) == dirs::home_dir() || parent == Path::new("/") {
            break;
        }
        if fs
            .read_dir(parent)
            .map(|children| children.is_empty())
            .unwrap_or(false)
        {
            let _ = fs.remove_dir(parent);
            dir = parent.parent();
        } else {
            break;
        }
    }
}
