//! Auto-detection of existing dotfile managers.
//!
//! Scans common dotfile repo locations and identifies what's managing them.

use std::path::{Path, PathBuf};

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
pub fn detect(home: &Path) -> Vec<DetectedManager> {
    let mut found = Vec::new();

    for rel in CANDIDATE_DIRS {
        let dir = home.join(rel);
        if dir.is_dir()
            && let Some(m) = probe(&dir)
        {
            found.push(m);
        }
    }

    // chezmoi source dir at its default non-standard location
    let chezmoi_src = home.join(".local/share/chezmoi");
    if chezmoi_src.is_dir() && !found.iter().any(|m| m.root == chezmoi_src) {
        found.push(DetectedManager {
            kind: ManagerKind::Chezmoi,
            packages: enumerate_chezmoi_packages(&chezmoi_src),
            root: chezmoi_src,
        });
    }

    found
}

fn probe(dir: &Path) -> Option<DetectedManager> {
    let kind = classify(dir);
    let packages = match kind {
        ManagerKind::Stow | ManagerKind::Unknown => enumerate_stow_packages(dir),
        ManagerKind::Chezmoi => enumerate_chezmoi_packages(dir),
        ManagerKind::Notfiles => enumerate_notfiles_packages(dir),
    };
    Some(DetectedManager {
        kind,
        root: dir.to_path_buf(),
        packages,
    })
}

fn classify(dir: &Path) -> ManagerKind {
    if dir.join("notfiles.toml").exists() {
        return ManagerKind::Notfiles;
    }
    if dir.join(".chezmoi.toml.tmpl").exists()
        || dir.join(".chezmoi.yaml.tmpl").exists()
        || dir.join(".chezmoi.json.tmpl").exists()
        || dir.join(".chezmoi.toml").exists()
        || dir.join(".chezmoi.yaml").exists()
    {
        return ManagerKind::Chezmoi;
    }
    if dir.join(".stow-local-ignore").exists()
        || dir.join(".stowrc").exists()
        || has_stow_structure(dir)
    {
        return ManagerKind::Stow;
    }
    ManagerKind::Unknown
}

/// Heuristic: looks like stow if ≥2 top-level subdirs contain dot-dirs or
/// known config paths (`.config/`, `.local/`, `Library/`, etc.).
fn has_stow_structure(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut stow_like = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let s = name.to_string_lossy();
        if s.starts_with('.') || matches!(s.as_ref(), "target" | "src") {
            continue;
        }
        if package_looks_like_stow(&path) {
            stow_like += 1;
            if stow_like >= 2 {
                return true;
            }
        }
    }
    false
}

fn package_looks_like_stow(pkg_dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(pkg_dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let s = name.to_string_lossy();
        if s.starts_with('.') || s == "Library" {
            return true;
        }
    }
    false
}

fn enumerate_stow_packages(root: &Path) -> Vec<DetectedPackage> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return vec![];
    };
    let mut pkgs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || matches!(name.as_str(), "target" | "src" | "docs" | "tests") {
            continue;
        }
        let files = walk_files(&path, &path);
        pkgs.push(DetectedPackage { name, files });
    }
    pkgs.sort_by(|a, b| a.name.cmp(&b.name));
    pkgs
}

fn enumerate_chezmoi_packages(root: &Path) -> Vec<DetectedPackage> {
    let files = walk_files(root, root);
    if files.is_empty() {
        vec![]
    } else {
        vec![DetectedPackage {
            name: "(source)".into(),
            files,
        }]
    }
}

fn enumerate_notfiles_packages(root: &Path) -> Vec<DetectedPackage> {
    let toml_path = root.join("notfiles.toml");
    if let Ok(content) = std::fs::read_to_string(&toml_path)
        && let Some(names) = parse_include_list(&content)
    {
        return names
            .into_iter()
            .map(|name| {
                let pkg_dir = root.join(&name);
                let files = walk_files(&pkg_dir, &pkg_dir);
                DetectedPackage { name, files }
            })
            .collect();
    }
    enumerate_stow_packages(root)
}

/// Minimal parse of `include = ["a", "b"]` from TOML.
fn parse_include_list(toml: &str) -> Option<Vec<String>> {
    let line = toml
        .lines()
        .find(|l| l.trim_start().starts_with("include"))?;
    let bracket_start = line.find('[')?;
    let bracket_end = line.find(']')?;
    let inner = &line[bracket_start + 1..bracket_end];
    let names: Vec<String> = inner
        .split(',')
        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() { None } else { Some(names) }
}

/// Recursively collect file paths relative to `base`.
fn walk_files(dir: &Path, base: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if matches!(name.as_str(), ".git" | ".DS_Store") {
            continue;
        }
        if path.is_dir() {
            files.extend(walk_files(&path, base));
        } else if path.is_file()
            && let Ok(rel) = path.strip_prefix(base)
        {
            files.push(rel.to_path_buf());
        }
    }
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
