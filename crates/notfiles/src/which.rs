//! Reverse lookup — map a managed target path back to its source package and file.
//!
//! Answers "where does `~/.gitconfig` come from?". Recorded state entries are
//! consulted first, then the symlink itself, then the packages configured in
//! `notfiles.toml` (which covers files that were never linked).

use std::path::{Component, Path, PathBuf};

use serde_json::json;

use crate::linker::State;
use crate::package::{collect_files_with_store, resolve_packages_filtered_with_store};
use crate::ports::FileStore;
use notcore::{Config, Method, NotfilesError, expand_tilde};

/// How a match was established.
#[derive(Debug, PartialEq)]
pub enum Origin {
    /// Recorded in `.notfiles-state.toml`.
    State,
    /// The target is a symlink pointing inside the dotfiles directory.
    Symlink,
    /// The target is where a configured package file would be placed.
    Config,
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Origin::State => write!(f, "state"),
            Origin::Symlink => write!(f, "symlink"),
            Origin::Config => write!(f, "config"),
        }
    }
}

pub struct WhichMatch {
    pub package: String,
    pub source: PathBuf,
    pub target: PathBuf,
    pub method: Method,
    pub origin: Origin,
    /// Whether the target currently reflects the source.
    pub current: bool,
}

/// Expand `~` and resolve a relative query against `cwd`, then clean away
/// `.` and `..` so the result can be compared against recorded paths.
pub fn normalize_target(query: &Path, cwd: &Path) -> Result<PathBuf, NotfilesError> {
    let expanded = expand_tilde(&query.to_string_lossy())?;
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    };
    Ok(lexical_clean(&absolute))
}

/// Resolve `.` and `..` components without touching the filesystem.
fn lexical_clean(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(component.as_os_str());
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Find the source package and file behind `target`.
///
/// `target` must already be absolute — see [`normalize_target`]. Returns every
/// match found by the first strategy that produces one, so a target claimed by
/// two packages is reported as such rather than silently resolved.
pub fn which(
    dotfiles_dir: &Path,
    config: &Config,
    state: &State,
    target: &Path,
    fs: &dyn FileStore,
) -> Vec<WhichMatch> {
    // Recorded state entries win — they carry the package and method used.
    let mut matches: Vec<WhichMatch> = state
        .entries
        .iter()
        .filter(|e| Path::new(&e.target) == target)
        .map(|e| {
            let source = PathBuf::from(&e.source);
            WhichMatch {
                package: e.package.clone(),
                current: reflects(&source, target, e.method, fs),
                source,
                target: target.to_path_buf(),
                method: e.method,
                origin: Origin::State,
            }
        })
        .collect();
    if !matches.is_empty() {
        return matches;
    }

    // No state entry — the target may still be a symlink into the dotfiles dir.
    if let Ok(link) = fs.read_link(target) {
        let resolved = if link.is_absolute() {
            lexical_clean(&link)
        } else {
            lexical_clean(&target.parent().unwrap_or(target).join(&link))
        };
        if let Ok(relative) = resolved.strip_prefix(dotfiles_dir)
            && let Some(first) = relative.components().next()
        {
            let package = first.as_os_str().to_string_lossy().to_string();
            matches.push(WhichMatch {
                method: config.method_for(&package),
                package,
                source: resolved,
                target: target.to_path_buf(),
                origin: Origin::Symlink,
                current: true,
            });
            return matches;
        }
    }

    // Nothing linked — report the package that would own this target.
    let Ok(packages) = resolve_packages_filtered_with_store(dotfiles_dir, &[], config, fs) else {
        return matches;
    };
    for package in packages {
        let Ok(target_base) = expand_tilde(config.target_for(&package)) else {
            continue;
        };
        let Ok(relative) = target.strip_prefix(&target_base) else {
            continue;
        };
        let package_dir = dotfiles_dir.join(&package);
        let Ok(files) = collect_files_with_store(&package_dir, config, &package, fs) else {
            continue;
        };
        if files.iter().any(|f| f == relative) {
            let source = package_dir.join(relative);
            let method = config.method_for(&package);
            matches.push(WhichMatch {
                package,
                current: reflects(&source, target, method, fs),
                source,
                target: target.to_path_buf(),
                method,
                origin: Origin::Config,
            });
        }
    }

    matches
}

/// Whether `target` currently carries what `source` says it should.
fn reflects(source: &Path, target: &Path, method: Method, fs: &dyn FileStore) -> bool {
    match method {
        Method::Symlink => fs.read_link(target).is_ok_and(|link| link == source),
        Method::Copy => match (fs.read(source), fs.read(target)) {
            (Ok(src), Ok(tgt)) => src == tgt,
            _ => false,
        },
    }
}

pub fn print_which(query: &Path, matches: &[WhichMatch]) {
    if matches.is_empty() {
        println!(
            "{}: \x1b[31mnot managed by notfiles\x1b[0m",
            query.display()
        );
        return;
    }
    for m in matches {
        let status = if m.current {
            "\x1b[32mcurrent\x1b[0m"
        } else {
            "\x1b[33mstale\x1b[0m"
        };
        println!("\x1b[1m{}\x1b[0m", m.target.display());
        println!("    package: {}", m.package);
        println!("    source:  {}", m.source.display());
        println!("    method:  {}", m.method);
        println!("    via:     {}", m.origin);
        println!("    status:  {status}");
    }
}

pub fn print_which_json(query: &Path, matches: &[WhichMatch]) {
    let items: Vec<_> = matches
        .iter()
        .map(|m| {
            json!({
                "package": m.package,
                "source": m.source.to_string_lossy(),
                "target": m.target.to_string_lossy(),
                "method": m.method.to_string(),
                "via": m.origin.to_string(),
                "current": m.current,
            })
        })
        .collect();
    let obj = json!({"query": query.to_string_lossy(), "matches": items});
    println!("{}", serde_json::to_string(&obj).unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::InMemoryFileStore;
    use crate::linker::StateEntry;

    fn config_with_target(target: &str) -> Config {
        toml::from_str(&format!("[defaults]\ntarget = \"{target}\"\n")).unwrap()
    }

    fn state_entry(package: &str, source: &str, target: &str, method: Method) -> StateEntry {
        StateEntry {
            package: package.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            method,
            linked_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_normalize_target_resolves_relative_and_dot_segments() {
        let cwd = Path::new("/home/u");
        assert_eq!(
            normalize_target(Path::new("./.gitconfig"), cwd).unwrap(),
            PathBuf::from("/home/u/.gitconfig")
        );
        assert_eq!(
            normalize_target(Path::new("/home/u/x/../.gitconfig"), cwd).unwrap(),
            PathBuf::from("/home/u/.gitconfig")
        );
    }

    #[test]
    fn test_which_finds_state_entry() {
        let fs = InMemoryFileStore::new();
        fs.add_file("/df/git/.gitconfig", b"[user]");
        fs.symlink(
            Path::new("/df/git/.gitconfig"),
            Path::new("/home/u/.gitconfig"),
        )
        .unwrap();

        let mut state = State::default();
        state.add_entry(state_entry(
            "git",
            "/df/git/.gitconfig",
            "/home/u/.gitconfig",
            Method::Symlink,
        ));

        let matches = which(
            Path::new("/df"),
            &config_with_target("/home/u"),
            &state,
            Path::new("/home/u/.gitconfig"),
            &fs,
        );

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].package, "git");
        assert_eq!(matches[0].source, PathBuf::from("/df/git/.gitconfig"));
        assert_eq!(matches[0].origin, Origin::State);
        assert!(matches[0].current);
    }

    #[test]
    fn test_which_marks_stale_when_symlink_points_elsewhere() {
        let fs = InMemoryFileStore::new();
        fs.add_file("/df/git/.gitconfig", b"[user]");
        fs.symlink(
            Path::new("/elsewhere/.gitconfig"),
            Path::new("/home/u/.gitconfig"),
        )
        .unwrap();

        let mut state = State::default();
        state.add_entry(state_entry(
            "git",
            "/df/git/.gitconfig",
            "/home/u/.gitconfig",
            Method::Symlink,
        ));

        let matches = which(
            Path::new("/df"),
            &config_with_target("/home/u"),
            &state,
            Path::new("/home/u/.gitconfig"),
            &fs,
        );

        assert_eq!(matches.len(), 1);
        assert!(!matches[0].current);
    }

    #[test]
    fn test_which_falls_back_to_reading_the_symlink() {
        let fs = InMemoryFileStore::new();
        fs.add_file("/df/zsh/.zshrc", b"# zshrc");
        fs.symlink(Path::new("/df/zsh/.zshrc"), Path::new("/home/u/.zshrc"))
            .unwrap();

        let matches = which(
            Path::new("/df"),
            &config_with_target("/home/u"),
            &State::default(),
            Path::new("/home/u/.zshrc"),
            &fs,
        );

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].package, "zsh");
        assert_eq!(matches[0].source, PathBuf::from("/df/zsh/.zshrc"));
        assert_eq!(matches[0].origin, Origin::Symlink);
        assert!(matches[0].current);
    }

    #[test]
    fn test_which_reports_unlinked_package_file_from_config() {
        let fs = InMemoryFileStore::new();
        fs.add_file("/df/zsh/.zshrc", b"# zshrc");

        let matches = which(
            Path::new("/df"),
            &config_with_target("/home/u"),
            &State::default(),
            Path::new("/home/u/.zshrc"),
            &fs,
        );

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].package, "zsh");
        assert_eq!(matches[0].origin, Origin::Config);
        assert!(!matches[0].current);
    }

    #[test]
    fn test_which_returns_nothing_for_unmanaged_path() {
        let fs = InMemoryFileStore::new();
        fs.add_file("/df/zsh/.zshrc", b"# zshrc");
        fs.add_file("/home/u/.bashrc", b"# bashrc");

        let matches = which(
            Path::new("/df"),
            &config_with_target("/home/u"),
            &State::default(),
            Path::new("/home/u/.bashrc"),
            &fs,
        );

        assert!(matches.is_empty());
    }
}
