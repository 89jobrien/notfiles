use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Filesystem boundary used to read manifests and shell configs.
pub trait Fs {
    fn read_to_string(&self, path: &Path) -> io::Result<String>;
    fn write(&self, path: &Path, contents: &str) -> io::Result<()>;
    /// Direct children of a directory. A missing directory is an error.
    fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>>;
}

/// Where a `cargo install`ed crate was built from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "location", rename_all = "lowercase")]
pub enum Source {
    Registry,
    Git(String),
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstalledCrate {
    pub name: String,
    pub version: String,
    pub source: Source,
}

/// Boundary for asking the machine what is installed.
pub trait ToolProbe {
    /// Whether an executable named `bin` resolves on `PATH`.
    fn on_path(&self, bin: &str) -> bool;
    /// Whether an absolute path exists.
    fn path_exists(&self, path: &Path) -> bool;
    fn installed_crates(&self) -> Vec<InstalledCrate>;
}

/// Boundary for installing missing tools. Each call returns whether the install succeeded.
pub trait Installer {
    fn mise_install(&self) -> io::Result<bool>;
    fn brew_install(&self, formula: &str) -> io::Result<bool>;
}
