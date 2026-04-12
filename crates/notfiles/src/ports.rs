use std::path::{Path, PathBuf};

/// FileStore trait abstracts file system I/O operations.
///
/// This trait defines the interface for all file system interactions in the notfiles
/// linker, enabling testing with mock implementations and potential future backends.
pub trait FileStore {
    /// Read entire file to string.
    fn read_to_string(&self, path: &Path) -> Result<String, std::io::Error>;

    /// Write content to file.
    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), std::io::Error>;

    /// Rename a file or directory.
    fn rename(&self, from: &Path, to: &Path) -> Result<(), std::io::Error>;

    /// Remove a file.
    fn remove_file(&self, path: &Path) -> Result<(), std::io::Error>;

    /// Recursively remove a directory and all its contents.
    fn remove_dir_all(&self, path: &Path) -> Result<(), std::io::Error>;

    /// Read the target of a symbolic link.
    fn read_link(&self, path: &Path) -> Result<PathBuf, std::io::Error>;

    /// Get metadata for a file or directory (without following symlinks).
    fn symlink_metadata(&self, path: &Path) -> Result<std::fs::Metadata, std::io::Error>;

    /// Get metadata for a file or directory (following symlinks).
    fn metadata(&self, path: &Path) -> Result<std::fs::Metadata, std::io::Error>;

    /// Create a directory and all missing parent directories.
    fn create_dir_all(&self, path: &Path) -> Result<(), std::io::Error>;

    /// Create a symbolic link at `link` pointing to `target`.
    ///
    /// On Unix systems, this creates a symlink. On Windows with symlink support enabled,
    /// this may also create a symlink. Fallback behavior (e.g., copying) is handled by
    /// the caller.
    #[cfg(unix)]
    fn symlink(&self, target: &Path, link: &Path) -> Result<(), std::io::Error>;

    /// Check if a path exists.
    fn exists(&self, path: &Path) -> bool;

    /// Check if a path is a directory.
    fn is_dir(&self, path: &Path) -> bool;
}
