use std::path::{Path, PathBuf};

use serde_json::json;

use crate::linker::State;
use crate::package::collect_files_with_store;
use crate::ports::FileStore;
use notcore::{Config, Method, expand_tilde};

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
    fs: &dyn FileStore,
) -> Vec<StatusEntry> {
    let mut results = Vec::new();
    let package_dir = dotfiles_dir.join(package);
    let method = config.method_for(package);
    let target_base = match expand_tilde(config.target_for(package)) {
        Ok(p) => p,
        Err(_) => return results,
    };

    // Check files that should exist
    if let Ok(files) = collect_files_with_store(&package_dir, config, package, fs) {
        for relative in &files {
            let source = package_dir.join(relative);
            let target = target_base.join(relative);
            let source_display = format!("{package}/{}", relative.display());

            let status = match method {
                Method::Symlink => {
                    if let Ok(link_target) = fs.read_link(&target) {
                        if link_target == source {
                            FileStatus::Linked
                        } else {
                            FileStatus::Conflict
                        }
                    } else if fs.exists(&target) {
                        FileStatus::Conflict
                    } else {
                        FileStatus::Missing
                    }
                }
                Method::Copy => {
                    let tracked = state.entries.iter().any(|e| {
                        e.package == package
                            && e.target == target.to_string_lossy().as_ref()
                            && e.source == source.to_string_lossy().as_ref()
                    });
                    let content_matches = match (fs.read(&source), fs.read(&target)) {
                        (Ok(source_bytes), Ok(target_bytes)) => source_bytes == target_bytes,
                        _ => false,
                    };
                    if tracked && content_matches {
                        FileStatus::Copied
                    } else if fs.exists(&target) {
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
        if !fs.exists(&source) {
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

#[derive(Debug, PartialEq)]
pub enum DiffKind {
    Identical,
    Modified,
    SourceOnly,
    TargetOnly,
}

pub struct DiffEntry {
    pub source_display: String,
    pub target: PathBuf,
    pub kind: DiffKind,
}

pub fn diff_package(
    dotfiles_dir: &Path,
    config: &Config,
    state: &State,
    package: &str,
    fs: &dyn FileStore,
) -> Vec<DiffEntry> {
    let mut results = Vec::new();
    let method = config.method_for(package);
    if method != Method::Copy {
        return results;
    }

    let package_dir = dotfiles_dir.join(package);
    let target_base = match expand_tilde(config.target_for(package)) {
        Ok(p) => p,
        Err(_) => return results,
    };

    // Check files tracked in state for this package
    for entry in state.entries_for_package(package) {
        let source = PathBuf::from(&entry.source);
        let target = PathBuf::from(&entry.target);
        let source_display = format!(
            "{package}/{}",
            source
                .strip_prefix(&package_dir)
                .unwrap_or(&source)
                .display()
        );

        let kind = match (fs.read(&source), fs.read(&target)) {
            (Ok(src), Ok(tgt)) => {
                if src == tgt {
                    DiffKind::Identical
                } else {
                    DiffKind::Modified
                }
            }
            (Ok(_), Err(_)) => DiffKind::SourceOnly,
            (Err(_), Ok(_)) => DiffKind::TargetOnly,
            (Err(_), Err(_)) => continue,
        };

        results.push(DiffEntry {
            source_display,
            target,
            kind,
        });
    }

    // Also check untracked files from the package dir
    if let Ok(files) = collect_files_with_store(&package_dir, config, package, fs) {
        for relative in &files {
            let source = package_dir.join(relative);
            let target = target_base.join(relative);
            // Skip if already covered by state entries
            if results.iter().any(|r| r.target == target) {
                continue;
            }
            let source_display = format!("{package}/{}", relative.display());
            let kind = if !fs.exists(&target) {
                DiffKind::SourceOnly
            } else {
                match (fs.read(&source), fs.read(&target)) {
                    (Ok(src), Ok(tgt)) if src == tgt => DiffKind::Identical,
                    (Ok(_), Ok(_)) => DiffKind::Modified,
                    _ => continue,
                }
            };
            results.push(DiffEntry {
                source_display,
                target,
                kind,
            });
        }
    }

    results
}

pub fn print_diff(package: &str, entries: &[DiffEntry]) {
    let non_identical: Vec<_> = entries
        .iter()
        .filter(|e| e.kind != DiffKind::Identical)
        .collect();
    if non_identical.is_empty() {
        println!("  {package}: all copies identical");
        return;
    }
    println!("  \x1b[1m{package}\x1b[0m:");
    for entry in &non_identical {
        let label = match entry.kind {
            DiffKind::Modified => "\x1b[33mmodified\x1b[0m",
            DiffKind::SourceOnly => "\x1b[36msource only\x1b[0m",
            DiffKind::TargetOnly => "\x1b[35mtarget only\x1b[0m",
            DiffKind::Identical => unreachable!(),
        };
        println!(
            "    {label} {} -> {}",
            entry.source_display,
            entry.target.display()
        );
    }
}

pub fn print_status(package: &str, entries: &[StatusEntry]) {
    if entries.is_empty() {
        println!("  {package}: (empty)");
        return;
    }
    println!("  \x1b[1m{package}\x1b[0m:");
    for entry in entries {
        println!(
            "    {} {} -> {}",
            entry.status,
            entry.source_display,
            entry.target.display()
        );
    }
}

pub fn print_status_json(package: &str, entries: &[StatusEntry]) {
    let items: Vec<_> = entries
        .iter()
        .map(|e| {
            json!({
                "source": e.source_display,
                "target": e.target.to_string_lossy(),
                "status": format!("{:?}", e.status).to_lowercase(),
            })
        })
        .collect();
    let obj = json!({"package": package, "entries": items});
    println!("{}", serde_json::to_string(&obj).unwrap_or_default());
}
