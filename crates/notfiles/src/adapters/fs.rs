use crate::ports::FileStore;
use std::path::{Path, PathBuf};

/// Standard file system adapter using std::fs.
pub struct FileStoreImpl;

impl FileStore for FileStoreImpl {
    fn read_to_string(&self, path: &Path) -> Result<String, std::io::Error> {
        std::fs::read_to_string(path)
    }

    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), std::io::Error> {
        std::fs::write(path, contents)
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), std::io::Error> {
        std::fs::rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> Result<(), std::io::Error> {
        std::fs::remove_file(path)
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), std::io::Error> {
        std::fs::remove_dir_all(path)
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, std::io::Error> {
        std::fs::read_link(path)
    }

    fn symlink_metadata(&self, path: &Path) -> Result<std::fs::Metadata, std::io::Error> {
        std::fs::symlink_metadata(path)
    }

    fn metadata(&self, path: &Path) -> Result<std::fs::Metadata, std::io::Error> {
        std::fs::metadata(path)
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(path)
    }

    #[cfg(unix)]
    fn symlink(&self, target: &Path, link: &Path) -> Result<(), std::io::Error> {
        std::os::unix::fs::symlink(target, link)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }
}
