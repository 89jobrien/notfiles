use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{Config, Method};
use crate::linker::State;
use crate::package::collect_files;
use crate::paths::expand_tilde;

#[derive(Debug, PartialEq)]
pub enum FileStatus {
    Linked,
    Copied,
    Missing,
    Conflict,
    Orphan,
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileStatus::Linked => write!(f, "\x1b[32mlinked\x1b[0m"),
            FileStatus::Copied => write!(f, "\x1b[32mcopied\x1b[0m"),
            FileStatus::Missing => write!(f, "\x1b[31mmissing\x1b[0m"),
            FileStatus::Conflict => write!(f, "\x1b[33mconflict\x1b[0m"),
            FileStatus::Orphan => write!(f, "\x1b[35morphan\x1b[0m"),
        }
    }
}

pub struct StatusEntry {
    pub source_display: String,
    pub target: PathBuf,
    pub status: FileStatus,
}

pub fn package_status(
    dotfiles_dir: &Path,
    config: &Config,
    state: &State,
    package: &str,
) -> Vec<StatusEntry> {
    let mut results = Vec::new();
    let package_dir = dotfiles_dir.join(package);
    let method = config.method_for(package);
    let target_base = match expand_tilde(config.target_for(package)) {
        Ok(p) => p,
        Err(_) => return results,
    };

    // Check files that should exist
    if let Ok(files) = collect_files(&package_dir, config, package) {
        for relative in &files {
            let source = package_dir.join(relative);
            let target = target_base.join(relative);
            let source_display = format!("{package}/{}", relative.display());

            let status = match method {
                Method::Symlink => {
                    if let Ok(link_target) = fs::read_link(&target) {
                        if link_target == source {
                            FileStatus::Linked
                        } else {
                            FileStatus::Conflict
                        }
                    } else if target.exists() {
                        FileStatus::Conflict
                    } else {
                        FileStatus::Missing
                    }
                }
                Method::Copy => {
                    let has_state = state.entries.iter().any(|e| {
                        e.package == package
                            && e.target == target.to_string_lossy().as_ref()
                    });
                    if has_state && target.exists() {
                        FileStatus::Copied
                    } else if target.exists() {
                        FileStatus::Conflict
                    } else {
                        FileStatus::Missing
                    }
                }
            };

            results.push(StatusEntry {
                source_display,
                target,
                status,
            });
        }
    }

    // Check for orphans: entries in state that no longer have a source file
    for entry in state.entries_for_package(package) {
        let source = PathBuf::from(&entry.source);
        if !source.exists() {
            let target = PathBuf::from(&entry.target);
            // Only add if we didn't already report this target
            if !results.iter().any(|r| r.target == target) {
                results.push(StatusEntry {
                    source_display: format!(
                        "{package}/{}",
                        source
                            .strip_prefix(&package_dir)
                            .unwrap_or(&source)
                            .display()
                    ),
                    target,
                    status: FileStatus::Orphan,
                });
            }
        }
    }

    results
}

pub fn print_status(package: &str, entries: &[StatusEntry]) {
    if entries.is_empty() {
        println!("  {package}: (empty)");
        return;
    }
    println!("  \x1b[1m{package}\x1b[0m:");
    for entry in entries {
        println!("    {} {} -> {}", entry.status, entry.source_display, entry.target.display());
    }
}
