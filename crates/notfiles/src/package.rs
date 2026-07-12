use std::path::{Path, PathBuf};

use crate::adapters::FileStoreImpl;
use crate::ignore::IgnoreMatcher;
use crate::ports::FileStore;
use notcore::{Config, NotfilesError};

/// Discover packages filtered by the config's include/exclude lists and
/// platform constraints.
pub fn discover_packages_filtered(
    dotfiles_dir: &Path,
    config: &Config,
) -> Result<Vec<String>, NotfilesError> {
    discover_packages_filtered_with_store(dotfiles_dir, config, &FileStoreImpl)
}

pub fn discover_packages_filtered_with_store(
    dotfiles_dir: &Path,
    config: &Config,
    fs: &dyn FileStore,
) -> Result<Vec<String>, NotfilesError> {
    let all = discover_packages_with_store(dotfiles_dir, fs)?;
    let filtered: Vec<String> = if let Some(include) = config.included_packages() {
        all.into_iter()
            .filter(|p| include.contains(&p.as_str()))
            .collect()
    } else {
        all.into_iter()
            .filter(|p| !config.is_package_excluded(p))
            .collect()
    };
    // Apply platform filter
    let filtered = filtered
        .into_iter()
        .filter(|p| config.is_package_for_current_platform(p))
        .collect();
    Ok(filtered)
}

/// Discover available packages (subdirectories of the dotfiles dir).
pub fn discover_packages(dotfiles_dir: &Path) -> Result<Vec<String>, NotfilesError> {
    discover_packages_with_store(dotfiles_dir, &FileStoreImpl)
}

pub fn discover_packages_with_store(
    dotfiles_dir: &Path,
    fs: &dyn FileStore,
) -> Result<Vec<String>, NotfilesError> {
    let mut packages = Vec::new();
    // TODO(#19): Replace implicit top-level directory discovery with an explicit
    // package contract so mixed-purpose repos do not accidentally link
    // operational directories like scripts/ or docs/.
    for path in fs.read_dir(dotfiles_dir)? {
        if fs.is_dir(&path) {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            // Skip hidden dirs and common non-package dirs
            if !name.starts_with('.') {
                packages.push(name);
            }
        }
    }
    packages.sort();
    Ok(packages)
}

/// Resolve which packages to operate on: if specific names given, validate them;
/// otherwise discover all.
pub fn resolve_packages(
    dotfiles_dir: &Path,
    requested: &[String],
) -> Result<Vec<String>, NotfilesError> {
    resolve_packages_with_store(dotfiles_dir, requested, &FileStoreImpl)
}

pub fn resolve_packages_with_store(
    dotfiles_dir: &Path,
    requested: &[String],
    fs: &dyn FileStore,
) -> Result<Vec<String>, NotfilesError> {
    resolve_packages_filtered_with_store(dotfiles_dir, requested, &Config::default(), fs)
}

pub fn resolve_packages_filtered_with_store(
    dotfiles_dir: &Path,
    requested: &[String],
    config: &Config,
    fs: &dyn FileStore,
) -> Result<Vec<String>, NotfilesError> {
    let available = discover_packages_filtered_with_store(dotfiles_dir, config, fs)?;
    if requested.is_empty() {
        return Ok(available);
    }
    for name in requested {
        if !available.contains(name) {
            let available_refs: Vec<&str> = available.iter().map(|s| s.as_str()).collect();
            let suggestion = notcore::suggest_package(name, &available_refs);
            let mut msg = name.clone();
            if let Some(s) = suggestion {
                msg = format!("{name} (did you mean '{s}'?)");
            }
            return Err(NotfilesError::PackageNotFound { name: msg });
        }
    }
    Ok(requested.to_vec())
}

/// Recursively walk a package directory and return all file paths (relative to the package dir).
pub fn collect_files(
    package_dir: &Path,
    config: &Config,
    package_name: &str,
) -> Result<Vec<PathBuf>, NotfilesError> {
    collect_files_with_store(package_dir, config, package_name, &FileStoreImpl)
}

pub fn collect_files_with_store(
    package_dir: &Path,
    config: &Config,
    package_name: &str,
    fs: &dyn FileStore,
) -> Result<Vec<PathBuf>, NotfilesError> {
    let patterns = config.ignore_patterns_for(package_name);
    let matcher = IgnoreMatcher::new(&patterns)?;
    let mut files = Vec::new();
    walk_dir(package_dir, package_dir, &matcher, &mut files, fs)?;
    files.sort();
    Ok(files)
}

fn walk_dir(
    base: &Path,
    current: &Path,
    matcher: &IgnoreMatcher,
    files: &mut Vec<PathBuf>,
    fs: &dyn FileStore,
) -> Result<(), NotfilesError> {
    for path in fs.read_dir(current)? {
        let relative = path.strip_prefix(base).unwrap().to_path_buf();

        if matcher.is_ignored(&relative) {
            continue;
        }

        if fs.is_dir(&path) {
            walk_dir(base, &path, matcher, files, fs)?;
        } else {
            files.push(relative);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_discover_packages() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("zsh")).unwrap();
        fs::create_dir(tmp.path().join("git")).unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();

        let pkgs = discover_packages(tmp.path()).unwrap();
        assert_eq!(pkgs, vec!["git", "zsh"]);
    }

    #[test]
    fn test_discover_respects_include_filter() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("git")).unwrap();
        fs::create_dir(tmp.path().join("zsh")).unwrap();
        fs::create_dir(tmp.path().join("scripts")).unwrap();

        let toml_str = r#"
[defaults]
target = "~"
include = ["git", "zsh"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let pkgs = discover_packages_filtered(tmp.path(), &config).unwrap();
        assert_eq!(pkgs, vec!["git", "zsh"]);
    }

    #[test]
    fn test_discover_respects_exclude_filter() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("git")).unwrap();
        fs::create_dir(tmp.path().join("scripts")).unwrap();
        fs::create_dir(tmp.path().join("docs")).unwrap();

        let toml_str = r#"
[defaults]
target = "~"
exclude = ["scripts", "docs"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let pkgs = discover_packages_filtered(tmp.path(), &config).unwrap();
        assert_eq!(pkgs, vec!["git"]);
    }

    #[test]
    fn test_resolve_specific_packages() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("zsh")).unwrap();
        fs::create_dir(tmp.path().join("git")).unwrap();

        let pkgs = resolve_packages(tmp.path(), &["zsh".into()]).unwrap();
        assert_eq!(pkgs, vec!["zsh"]);
    }

    #[test]
    fn test_resolve_missing_package() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("zsh")).unwrap();

        let err = resolve_packages(tmp.path(), &["nope".into()]).unwrap_err();
        assert!(err.to_string().contains("nope"));
    }

    #[test]
    fn test_collect_files_with_ignore() {
        let tmp = TempDir::new().unwrap();
        let pkg = tmp.path().join("zsh");
        fs::create_dir_all(pkg.join(".config/zsh")).unwrap();
        fs::write(pkg.join(".config/zsh/zshrc"), "# zshrc").unwrap();
        fs::write(pkg.join("README.md"), "readme").unwrap();
        fs::write(pkg.join(".DS_Store"), "junk").unwrap();

        let config = Config::default();
        let files = collect_files(&pkg, &config, "zsh").unwrap();
        assert_eq!(files, vec![PathBuf::from(".config/zsh/zshrc")]);
    }
}
