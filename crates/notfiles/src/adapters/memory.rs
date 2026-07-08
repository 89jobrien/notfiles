use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::ports::FileStore;

/// In-memory file system for testing. Not thread-safe.
pub struct InMemoryFileStore {
    files: RefCell<HashMap<PathBuf, Vec<u8>>>,
    dirs: RefCell<HashSet<PathBuf>>,
    symlinks: RefCell<HashMap<PathBuf, PathBuf>>,
}

impl InMemoryFileStore {
    pub fn new() -> Self {
        Self {
            files: RefCell::new(HashMap::new()),
            dirs: RefCell::new(HashSet::new()),
            symlinks: RefCell::new(HashMap::new()),
        }
    }

    pub fn add_file(&self, path: impl AsRef<Path>, content: &[u8]) {
        let path = path.as_ref().to_path_buf();
        // Ensure parent dirs exist
        if let Some(parent) = path.parent() {
            self.ensure_parents(parent);
        }
        self.files.borrow_mut().insert(path, content.to_vec());
    }

    pub fn add_dir(&self, path: impl AsRef<Path>) {
        self.ensure_parents(path.as_ref());
    }

    pub fn has_symlink(&self, link: &Path) -> bool {
        self.symlinks.borrow().contains_key(link)
    }

    pub fn symlink_target(&self, link: &Path) -> Option<PathBuf> {
        self.symlinks.borrow().get(link).cloned()
    }

    fn ensure_parents(&self, path: &Path) {
        let mut current = path.to_path_buf();
        let mut dirs = self.dirs.borrow_mut();
        loop {
            if !dirs.insert(current.clone()) {
                break;
            }
            match current.parent() {
                Some(p) if p != current => current = p.to_path_buf(),
                _ => break,
            }
        }
    }
}

impl Default for InMemoryFileStore {
    fn default() -> Self {
        Self::new()
    }
}

impl FileStore for InMemoryFileStore {
    fn read(&self, path: &Path) -> Result<Vec<u8>, std::io::Error> {
        // Follow symlinks
        if let Some(target) = self.symlinks.borrow().get(path) {
            return self.read(target);
        }
        self.files.borrow().get(path).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, path.display().to_string())
        })
    }

    fn read_to_string(&self, path: &Path) -> Result<String, std::io::Error> {
        let bytes = self.read(path)?;
        String::from_utf8(bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), std::io::Error> {
        if let Some(parent) = path.parent() {
            self.ensure_parents(parent);
        }
        self.files
            .borrow_mut()
            .insert(path.to_path_buf(), contents.to_vec());
        Ok(())
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), std::io::Error> {
        let content = self.files.borrow_mut().remove(from);
        if let Some(content) = content {
            if let Some(parent) = to.parent() {
                self.ensure_parents(parent);
            }
            self.files.borrow_mut().insert(to.to_path_buf(), content);
            return Ok(());
        }
        if self.symlinks.borrow_mut().remove(from).is_some() {
            return Ok(());
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            from.display().to_string(),
        ))
    }

    fn remove_file(&self, path: &Path) -> Result<(), std::io::Error> {
        if self.files.borrow_mut().remove(path).is_some() {
            return Ok(());
        }
        if self.symlinks.borrow_mut().remove(path).is_some() {
            return Ok(());
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            path.display().to_string(),
        ))
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), std::io::Error> {
        let path = path.to_path_buf();
        self.files.borrow_mut().retain(|k, _| !k.starts_with(&path));
        self.dirs.borrow_mut().retain(|k| !k.starts_with(&path));
        self.symlinks
            .borrow_mut()
            .retain(|k, _| !k.starts_with(&path));
        Ok(())
    }

    fn remove_dir(&self, path: &Path) -> Result<(), std::io::Error> {
        self.dirs.borrow_mut().remove(path);
        Ok(())
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, std::io::Error> {
        self.symlinks.borrow().get(path).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, path.display().to_string())
        })
    }

    fn symlink_metadata(&self, path: &Path) -> Result<std::fs::Metadata, std::io::Error> {
        // We can't construct real Metadata, but we can indicate existence
        // by returning NotFound when the path doesn't exist
        if self.files.borrow().contains_key(path)
            || self.dirs.borrow().contains(path)
            || self.symlinks.borrow().contains_key(path)
        {
            // Return metadata of a real temp file as a stand-in
            // This is a known limitation of the in-memory store
            Err(std::io::Error::other(
                "InMemoryFileStore: metadata not available",
            ))
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                path.display().to_string(),
            ))
        }
    }

    fn metadata(&self, path: &Path) -> Result<std::fs::Metadata, std::io::Error> {
        self.symlink_metadata(path)
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), std::io::Error> {
        self.ensure_parents(path);
        Ok(())
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
        let path = path.to_path_buf();
        let mut children = Vec::new();

        for key in self.files.borrow().keys() {
            if key.parent() == Some(&path) {
                children.push(key.clone());
            }
        }

        for dir in self.dirs.borrow().iter() {
            if dir.parent() == Some(&path) && dir != &path && !children.contains(dir) {
                children.push(dir.clone());
            }
        }

        for key in self.symlinks.borrow().keys() {
            if key.parent() == Some(&path) && !children.contains(key) {
                children.push(key.clone());
            }
        }

        Ok(children)
    }

    #[cfg(unix)]
    fn symlink(&self, target: &Path, link: &Path) -> Result<(), std::io::Error> {
        if let Some(parent) = link.parent() {
            self.ensure_parents(parent);
        }
        self.symlinks
            .borrow_mut()
            .insert(link.to_path_buf(), target.to_path_buf());
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.borrow().contains_key(path)
            || self.dirs.borrow().contains(path)
            || self.symlinks.borrow().contains_key(path)
    }

    fn is_dir(&self, path: &Path) -> bool {
        self.dirs.borrow().contains(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_file_creates_parents() {
        let fs = InMemoryFileStore::new();
        fs.add_file("/a/b/c.txt", b"hello");
        assert!(fs.is_dir(Path::new("/a")));
        assert!(fs.is_dir(Path::new("/a/b")));
        assert!(fs.exists(Path::new("/a/b/c.txt")));
    }

    #[test]
    fn test_read_write_roundtrip() {
        let fs = InMemoryFileStore::new();
        fs.write(Path::new("/tmp/f.txt"), b"data").unwrap();
        assert_eq!(fs.read(Path::new("/tmp/f.txt")).unwrap(), b"data");
        assert_eq!(fs.read_to_string(Path::new("/tmp/f.txt")).unwrap(), "data");
    }

    #[test]
    fn test_symlink_roundtrip() {
        let fs = InMemoryFileStore::new();
        fs.add_file("/src/config", b"content");
        fs.symlink(Path::new("/src/config"), Path::new("/home/config"))
            .unwrap();
        assert!(fs.has_symlink(Path::new("/home/config")));
        assert_eq!(
            fs.read_link(Path::new("/home/config")).unwrap(),
            PathBuf::from("/src/config")
        );
        // Reading through symlink
        assert_eq!(fs.read(Path::new("/home/config")).unwrap(), b"content");
    }

    #[test]
    fn test_rename_file() {
        let fs = InMemoryFileStore::new();
        fs.add_file("/a.txt", b"hello");
        fs.rename(Path::new("/a.txt"), Path::new("/b.txt")).unwrap();
        assert!(!fs.exists(Path::new("/a.txt")));
        assert_eq!(fs.read(Path::new("/b.txt")).unwrap(), b"hello");
    }

    #[test]
    fn test_remove_dir_all() {
        let fs = InMemoryFileStore::new();
        fs.add_file("/dir/a.txt", b"a");
        fs.add_file("/dir/sub/b.txt", b"b");
        fs.remove_dir_all(Path::new("/dir")).unwrap();
        assert!(!fs.exists(Path::new("/dir/a.txt")));
        assert!(!fs.exists(Path::new("/dir/sub/b.txt")));
        assert!(!fs.is_dir(Path::new("/dir")));
    }

    #[test]
    fn test_read_dir_lists_children() {
        let fs = InMemoryFileStore::new();
        fs.add_file("/dir/a.txt", b"a");
        fs.add_file("/dir/b.txt", b"b");
        fs.add_dir("/dir/sub");
        let mut children = fs.read_dir(Path::new("/dir")).unwrap();
        children.sort();
        assert_eq!(
            children,
            vec![
                PathBuf::from("/dir/a.txt"),
                PathBuf::from("/dir/b.txt"),
                PathBuf::from("/dir/sub"),
            ]
        );
    }
}
