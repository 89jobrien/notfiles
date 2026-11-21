use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ports::{Fs, InstalledCrate, Installer, Source, ToolProbe};

// ---------------------------------------------------------------------------
// System adapters
// ---------------------------------------------------------------------------

pub struct SystemFs;

impl Fs for SystemFs {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn write(&self, path: &Path, contents: &str) -> io::Result<()> {
        std::fs::write(path, contents)
    }

    fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let mut out: Vec<PathBuf> = std::fs::read_dir(path)?
            .flatten()
            .map(|e| e.path())
            .collect();
        out.sort();
        Ok(out)
    }
}

pub struct SystemProbe;

impl ToolProbe for SystemProbe {
    fn on_path(&self, bin: &str) -> bool {
        std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
            .unwrap_or(false)
    }

    fn path_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn installed_crates(&self) -> Vec<InstalledCrate> {
        Command::new("cargo")
            .args(["install", "--list"])
            .output()
            .map(|o| parse_cargo_install_list(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
    }
}

/// Parse `cargo install --list`: header lines look like `name v1.2.3:` or
/// `name v1.2.3 (/path/or/https://git/url#sha):`; indented lines are binaries.
pub fn parse_cargo_install_list(text: &str) -> Vec<InstalledCrate> {
    text.lines()
        .filter(|l| !l.is_empty() && !l.starts_with(char::is_whitespace))
        .filter_map(|l| {
            let source = match l
                .split_once('(')
                .map(|(_, s)| s.trim_end_matches([':', ')']))
            {
                None => Source::Registry,
                Some(s) if s.contains("://") => {
                    Source::Git(s.split('#').next().unwrap_or(s).to_string())
                }
                Some(_) => Source::Path,
            };
            let mut words = l.trim_end_matches(':').split_whitespace();
            let name = words.next()?.to_string();
            let version = words.next()?.trim_start_matches('v').to_string();
            Some(InstalledCrate {
                name,
                version,
                source,
            })
        })
        .collect()
}

pub struct SystemInstaller;

impl Installer for SystemInstaller {
    fn mise_install(&self) -> io::Result<bool> {
        Ok(Command::new("mise").arg("install").status()?.success())
    }

    fn brew_install(&self, formula: &str) -> io::Result<bool> {
        Ok(Command::new("brew")
            .args(["install", formula])
            .status()?
            .success())
    }
}

// ---------------------------------------------------------------------------
// In-memory adapters for tests
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct MemoryFs {
    files: RefCell<BTreeMap<PathBuf, String>>,
}

impl MemoryFs {
    pub fn new(files: &[(&str, &str)]) -> Self {
        let map = files
            .iter()
            .map(|(p, c)| (PathBuf::from(p), c.to_string()))
            .collect();
        Self {
            files: RefCell::new(map),
        }
    }

    pub fn get(&self, path: &str) -> Option<String> {
        self.files.borrow().get(Path::new(path)).cloned()
    }
}

impl Fs for MemoryFs {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        self.get(&path.to_string_lossy())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, path.display().to_string()))
    }

    fn write(&self, path: &Path, contents: &str) -> io::Result<()> {
        self.files
            .borrow_mut()
            .insert(path.to_path_buf(), contents.to_string());
        Ok(())
    }

    fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let children: Vec<PathBuf> = self
            .files
            .borrow()
            .keys()
            .filter(|p| p.parent() == Some(path))
            .cloned()
            .collect();
        if children.is_empty() {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                path.display().to_string(),
            ))
        } else {
            Ok(children)
        }
    }
}

pub struct FakeProbe {
    bins: BTreeSet<String>,
    paths: BTreeSet<PathBuf>,
    crates: Vec<InstalledCrate>,
}

impl FakeProbe {
    pub fn new(bins: &[&str], paths: &[&str], crates: Vec<InstalledCrate>) -> Self {
        Self {
            bins: bins.iter().map(|s| s.to_string()).collect(),
            paths: paths.iter().map(PathBuf::from).collect(),
            crates,
        }
    }
}

impl ToolProbe for FakeProbe {
    fn on_path(&self, bin: &str) -> bool {
        self.bins.contains(bin)
    }

    fn path_exists(&self, path: &Path) -> bool {
        self.paths.contains(path)
    }

    fn installed_crates(&self) -> Vec<InstalledCrate> {
        self.crates.clone()
    }
}

pub struct RecordingInstaller {
    ok: bool,
    calls: RefCell<Vec<String>>,
}

impl RecordingInstaller {
    pub fn new(ok: bool) -> Self {
        Self {
            ok,
            calls: RefCell::new(Vec::new()),
        }
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl Installer for RecordingInstaller {
    fn mise_install(&self) -> io::Result<bool> {
        self.calls.borrow_mut().push("mise install".into());
        Ok(self.ok)
    }

    fn brew_install(&self, formula: &str) -> io::Result<bool> {
        self.calls
            .borrow_mut()
            .push(format!("brew install {formula}"));
        Ok(self.ok)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cargo_install_list() {
        let text = "bacon v3.22.0:\n    bacon\nnotfiles v0.1.0 (/Users/j/notfiles/crates/notfiles):\n    notfiles\nslsp v0.1.0 (https://git.example.com/estin/slsp.git#abc123):\n    slsp\n";
        let got = parse_cargo_install_list(text);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].source, Source::Registry);
        assert_eq!(got[0].version, "3.22.0");
        assert_eq!(got[1].source, Source::Path);
        assert_eq!(
            got[2].source,
            Source::Git("https://git.example.com/estin/slsp.git".into())
        );
    }

    #[test]
    fn memory_fs_lists_children_only() {
        let fs = MemoryFs::new(&[("/a/x", "1"), ("/a/y", "2"), ("/b/z", "3")]);
        assert_eq!(fs.list_dir(Path::new("/a")).unwrap().len(), 2);
        assert!(fs.list_dir(Path::new("/c")).is_err());
    }
}
