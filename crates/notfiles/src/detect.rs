//! Auto-detection of existing dotfile managers.
//!
//! Scans common dotfile repo locations and identifies what's managing them.

use std::path::{Path, PathBuf};

use crate::ports::FileStore;

use serde_json::json;

/// A dotfile manager detected on disk.
#[derive(Debug)]
pub struct DetectedManager {
    pub kind: ManagerKind,
    pub root: PathBuf,
    pub packages: Vec<DetectedPackage>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ManagerKind {
    /// GNU Stow — packages are top-level dirs, files mirror $HOME layout.
    Stow,
    /// chezmoi — uses `.chezmoi.*` config and `dot_` prefix convention.
    Chezmoi,
    /// Notfiles — has a `notfiles.toml` config file.
    Notfiles,
    /// Unknown structure — looks like a dotfile repo but manager unclear.
    Unknown,
}

impl std::fmt::Display for ManagerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManagerKind::Stow => write!(f, "stow"),
            ManagerKind::Chezmoi => write!(f, "chezmoi"),
            ManagerKind::Notfiles => write!(f, "notfiles"),
            ManagerKind::Unknown => write!(f, "unknown"),
        }
    }
}

/// A package (top-level directory) inside a detected dotfile repo.
#[derive(Debug)]
pub struct DetectedPackage {
    pub name: String,
    /// Files relative to the package directory (mirrors home layout).
    pub files: Vec<PathBuf>,
}

/// Locations to probe relative to `$HOME`.
const CANDIDATE_DIRS: &[&str] = &[
    "dotfiles",
    ".dotfiles",
    "dot",
    ".dot",
    "dots",
    ".dots",
    "config/dotfiles",
    ".config/dotfiles",
];

/// Detect all dotfile managers reachable from `home`.
pub fn detect(store: &dyn FileStore, home: &Path) -> Vec<DetectedManager> {
    let mut found = Vec::new();

    for rel in CANDIDATE_DIRS {
        let dir = home.join(rel);
        if store.is_dir(&dir) {
            found.push(probe(store, &dir));
        }
    }

    // chezmoi source dir at its default non-standard location
    let chezmoi_src = home.join(".local/share/chezmoi");
    if store.is_dir(&chezmoi_src) && !found.iter().any(|m| m.root == chezmoi_src) {
        found.push(DetectedManager {
            kind: ManagerKind::Chezmoi,
            packages: enumerate_chezmoi_packages(store, &chezmoi_src),
            root: chezmoi_src,
        });
    }

    found
}

fn probe(store: &dyn FileStore, dir: &Path) -> DetectedManager {
    let kind = classify(store, dir);
    let packages = match kind {
        ManagerKind::Stow | ManagerKind::Unknown => enumerate_stow_packages(store, dir),
        ManagerKind::Chezmoi => enumerate_chezmoi_packages(store, dir),
        ManagerKind::Notfiles => enumerate_notfiles_packages(store, dir),
    };
    DetectedManager {
        kind,
        root: dir.to_path_buf(),
        packages,
    }
}

fn classify(store: &dyn FileStore, dir: &Path) -> ManagerKind {
    let has = |name: &str| store.exists(&dir.join(name));
    if has("notfiles.toml") {
        return ManagerKind::Notfiles;
    }
    if [
        ".chezmoi.toml.tmpl",
        ".chezmoi.yaml.tmpl",
        ".chezmoi.json.tmpl",
        ".chezmoi.toml",
        ".chezmoi.yaml",
    ]
    .iter()
    .any(|n| has(n))
    {
        return ManagerKind::Chezmoi;
    }
    if has(".stow-local-ignore") || has(".stowrc") || has_stow_structure(store, dir) {
        return ManagerKind::Stow;
    }
    ManagerKind::Unknown
}

/// File or directory name of `path`, lossily decoded.
fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Heuristic: looks like stow if ≥2 top-level subdirs contain dot-dirs or
/// known config paths (`.config/`, `.local/`, `Library/`, etc.).
fn has_stow_structure(store: &dyn FileStore, dir: &Path) -> bool {
    let Ok(entries) = store.read_dir(dir) else {
        return false;
    };
    let mut stow_like = 0usize;
    for path in entries {
        if !store.is_dir(&path) {
            continue;
        }
        let name = name_of(&path);
        if name.starts_with('.') || matches!(name.as_str(), "target" | "src") {
            continue;
        }
        if package_looks_like_stow(store, &path) {
            stow_like += 1;
            if stow_like >= 2 {
                return true;
            }
        }
    }
    false
}

fn package_looks_like_stow(store: &dyn FileStore, pkg_dir: &Path) -> bool {
    store.read_dir(pkg_dir).is_ok_and(|entries| {
        entries.iter().any(|p| {
            let name = name_of(p);
            name.starts_with('.') || name == "Library"
        })
    })
}

fn enumerate_stow_packages(store: &dyn FileStore, root: &Path) -> Vec<DetectedPackage> {
    let Ok(entries) = store.read_dir(root) else {
        return vec![];
    };
    let mut pkgs = Vec::new();
    for path in entries {
        if !store.is_dir(&path) {
            continue;
        }
        let name = name_of(&path);
        if name.starts_with('.') || matches!(name.as_str(), "target" | "src" | "docs" | "tests") {
            continue;
        }
        let files = walk_files(store, &path, &path);
        pkgs.push(DetectedPackage { name, files });
    }
    pkgs.sort_by(|a, b| a.name.cmp(&b.name));
    pkgs
}

fn enumerate_chezmoi_packages(store: &dyn FileStore, root: &Path) -> Vec<DetectedPackage> {
    let files = walk_files(store, root, root);
    if files.is_empty() {
        vec![]
    } else {
        vec![DetectedPackage {
            name: "(source)".into(),
            files,
        }]
    }
}

fn enumerate_notfiles_packages(store: &dyn FileStore, root: &Path) -> Vec<DetectedPackage> {
    let toml_path = root.join("notfiles.toml");
    if let Ok(content) = store.read_to_string(&toml_path)
        && let Some(names) = parse_include_list(&content)
    {
        return names
            .into_iter()
            .map(|name| {
                let pkg_dir = root.join(&name);
                let files = walk_files(store, &pkg_dir, &pkg_dir);
                DetectedPackage { name, files }
            })
            .collect();
    }
    enumerate_stow_packages(store, root)
}

/// Minimal parse of `include = ["a", "b"]` from TOML.
fn parse_include_list(toml: &str) -> Option<Vec<String>> {
    let line = toml
        .lines()
        .find(|l| l.trim_start().starts_with("include"))?;
    let after_open = &line[line.find('[')? + 1..];
    let inner = &after_open[..after_open.find(']')?];
    let names: Vec<String> = inner
        .split(',')
        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() { None } else { Some(names) }
}

/// Recursively collect file paths relative to `base`, sorted for stable output.
fn walk_files(store: &dyn FileStore, dir: &Path, base: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = store.read_dir(dir) else {
        return files;
    };
    for path in entries {
        if matches!(name_of(&path).as_str(), ".git" | ".DS_Store") {
            continue;
        }
        if store.is_dir(&path) {
            files.extend(walk_files(store, &path, base));
        } else if store.exists(&path)
            && let Ok(rel) = path.strip_prefix(base)
        {
            files.push(rel.to_path_buf());
        }
    }
    files.sort();
    files
}

// ── Output ────────────────────────────────────────────────────────────────────

pub fn print_detected(managers: &[DetectedManager]) {
    if managers.is_empty() {
        println!("No dotfile managers detected.");
        return;
    }
    println!("Detected dotfile managers:\n");
    for m in managers {
        let pkg_count = m.packages.len();
        let file_count: usize = m.packages.iter().map(|p| p.files.len()).sum();
        let pkg_names: Vec<&str> = m.packages.iter().map(|p| p.name.as_str()).collect();
        println!("  \x1b[1m{}\x1b[0m  {}", m.kind, m.root.display());
        println!(
            "    {} package{}, {} file{}",
            pkg_count,
            if pkg_count == 1 { "" } else { "s" },
            file_count,
            if file_count == 1 { "" } else { "s" },
        );
        if !pkg_names.is_empty() {
            println!("    packages: {}", pkg_names.join(", "));
        }
        println!();
    }
}

pub fn print_detected_json(managers: &[DetectedManager]) {
    let items: Vec<_> = managers
        .iter()
        .map(|m| {
            json!({
                "manager": m.kind.to_string(),
                "root": m.root.to_string_lossy(),
                "packages": m.packages.iter().map(|p| {
                    json!({
                        "name": p.name,
                        "files": p.files.iter()
                            .map(|f| f.to_string_lossy().to_string())
                            .collect::<Vec<_>>(),
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&items).unwrap_or_default()
    );
}

#[cfg(test)]
mod tests {
    use super::parse_include_list;

    #[test]
    fn parses_double_and_single_quoted_names() {
        assert_eq!(
            parse_include_list("include = [\"zsh\", 'git' , nu]\n"),
            Some(vec!["zsh".into(), "git".into(), "nu".into()])
        );
    }

    #[test]
    fn empty_or_missing_include_is_none() {
        assert_eq!(parse_include_list("include = []\n"), None);
        assert_eq!(parse_include_list("[defaults]\nmethod = \"copy\"\n"), None);
    }

    /// Regression: `]` before `[` made the slice bounds cross and panicked.
    #[test]
    fn closing_bracket_before_opening_bracket_is_none() {
        assert_eq!(parse_include_list("include = ]x["), None);
        assert_eq!(parse_include_list("include]["), None);
    }

    #[test]
    fn unterminated_list_is_none() {
        assert_eq!(parse_include_list("include = [\"zsh\"\n"), None);
    }

    #[test]
    fn only_the_first_include_line_counts() {
        assert_eq!(
            parse_include_list("include = [\"a\"]\ninclude = [\"b\"]\n"),
            Some(vec!["a".into()])
        );
    }
}

#[cfg(test)]
mod properties {
    use super::parse_include_list;
    use proptest::prelude::*;

    proptest! {
        /// Whatever the manifest holds, parsing must not panic.
        #[test]
        fn parse_include_list_never_panics(text in "\\PC*") {
            let _ = parse_include_list(&text);
        }

        /// Same, biased towards the bracket and quote characters the parser slices on.
        #[test]
        fn parse_include_list_never_panics_on_bracket_soup(soup in "[\\[\\]\",' a-z=\\n]{0,40}") {
            // Anchor on the `include` key so the bracket slicing is always reached.
            let _ = parse_include_list(&format!("include{soup}"));
        }

        /// A well-formed list returns exactly the names written.
        #[test]
        fn well_formed_lists_round_trip(
            names in prop::collection::vec("[a-z][a-z0-9_-]{0,8}", 1..6),
        ) {
            let quoted: Vec<String> = names.iter().map(|n| format!("\"{n}\"")).collect();
            let text = format!("include = [{}]\n", quoted.join(", "));
            prop_assert_eq!(parse_include_list(&text), Some(names));
        }
    }
}
