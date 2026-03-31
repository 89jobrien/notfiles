use std::fs;
use std::path::{Path, PathBuf};

use notcore::{Config, NotfilesError};
use crate::ignore::IgnoreMatcher;

/// Discover available packages (subdirectories of the dotfiles dir).
pub fn discover_packages(dotfiles_dir: &Path) -> Result<Vec<String>, NotfilesError> {
    let mut packages = Vec::new();
    for entry in fs::read_dir(dotfiles_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
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
    let available = discover_packages(dotfiles_dir)?;
    if requested.is_empty() {
        return Ok(available);
    }
    for name in requested {
        if !available.contains(name) {
            return Err(NotfilesError::PackageNotFound { name: name.clone() });
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
    let patterns = config.ignore_patterns_for(package_name);
    let matcher = IgnoreMatcher::new(&patterns)?;
    let mut files = Vec::new();
    walk_dir(package_dir, package_dir, &matcher, &mut files)?;
    files.sort();
    Ok(files)
}

fn walk_dir(
    base: &Path,
    current: &Path,
    matcher: &IgnoreMatcher,
    files: &mut Vec<PathBuf>,
) -> Result<(), NotfilesError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(base).unwrap().to_path_buf();

        if matcher.is_ignored(&relative) {
            continue;
        }

        if path.is_dir() {
            walk_dir(base, &path, matcher, files)?;
        } else {
            files.push(relative);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
