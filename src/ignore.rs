use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::Path;

use crate::error::NotfilesError;

pub struct IgnoreMatcher {
    globset: GlobSet,
}

impl IgnoreMatcher {
    pub fn new(patterns: &[&str]) -> Result<Self, NotfilesError> {
        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            // Match the pattern as a filename component and also as a path suffix.
            let glob = Glob::new(pattern)
                .or_else(|_| Glob::new(&format!("**/{pattern}")))
                .map_err(|e| NotfilesError::Other(format!("invalid ignore pattern '{pattern}': {e}")))?;
            builder.add(glob);
            // Also add a recursive variant so "foo" matches "a/foo" etc.
            if !pattern.contains('/') && !pattern.starts_with("**/") {
                if let Ok(g) = Glob::new(&format!("**/{pattern}")) {
                    builder.add(g);
                }
            }
        }
        let globset = builder
            .build()
            .map_err(|e| NotfilesError::Other(format!("building ignore set: {e}")))?;
        Ok(Self { globset })
    }

    /// Check if a relative path should be ignored.
    pub fn is_ignored(&self, relative_path: &Path) -> bool {
        if self.globset.is_match(relative_path) {
            return true;
        }
        // Also check each component individually (for directory-level ignores like ".git").
        for component in relative_path.components() {
            if self.globset.is_match(component.as_os_str()) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_basic_ignore() {
        let m = IgnoreMatcher::new(&[".git", "README.md"]).unwrap();
        assert!(m.is_ignored(Path::new(".git")));
        assert!(m.is_ignored(Path::new("README.md")));
        assert!(!m.is_ignored(Path::new("config")));
    }

    #[test]
    fn test_nested_ignore() {
        let m = IgnoreMatcher::new(&[".DS_Store"]).unwrap();
        assert!(m.is_ignored(Path::new(".DS_Store")));
        assert!(m.is_ignored(Path::new("subdir/.DS_Store")));
    }

    #[test]
    fn test_glob_pattern() {
        let m = IgnoreMatcher::new(&["*.bak"]).unwrap();
        assert!(m.is_ignored(Path::new("file.bak")));
        assert!(m.is_ignored(Path::new("subdir/file.bak")));
        assert!(!m.is_ignored(Path::new("file.txt")));
    }

    #[test]
    fn test_directory_component_ignore() {
        let m = IgnoreMatcher::new(&[".git"]).unwrap();
        assert!(m.is_ignored(Path::new(".git/config")));
        assert!(m.is_ignored(Path::new(".git/objects/abc")));
    }
}
