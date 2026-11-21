//! Post-link tool reconciliation: install what the dotfiles manifest declares, then report
//! alias targets that are declared nowhere.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use nottools::adapters::{SystemFs, SystemInstaller, SystemProbe};
use nottools::{DoctorConfig, Fs, Installer, ToolProbe};

/// What the reconciliation did and what is still wrong.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ToolsSummary {
    /// Install commands that succeeded.
    pub installed: Vec<String>,
    /// Declared tools whose install failed or that still do not resolve.
    pub unresolved: Vec<String>,
    /// Alias targets declared in no manifest; nothing to install.
    pub undeclared: Vec<String>,
}

/// Reconcile tools for the dotfiles checkout at `dotfiles_dir`.
/// Returns `None` when its `notfiles.toml` has no `[doctor]` table.
pub fn reconcile(
    dotfiles_dir: &Path,
    home: &Path,
    fs: &dyn Fs,
    probe: &dyn ToolProbe,
    installer: &dyn Installer,
) -> Result<Option<ToolsSummary>> {
    let toml_path = dotfiles_dir.join("notfiles.toml");
    let text = fs
        .read_to_string(&toml_path)
        .with_context(|| format!("reading {}", toml_path.display()))?;
    let Some(cfg) = DoctorConfig::parse(&text)? else {
        return Ok(None);
    };

    let first = nottools::run(&cfg, dotfiles_dir, home, fs, probe)?;
    let actions = nottools::apply_fixes(&first, installer, false);
    let mut summary = ToolsSummary::default();
    for (action, ok) in &actions {
        if *ok {
            summary.installed.push(action.describe());
        } else {
            summary.unresolved.push(action.describe());
        }
    }

    let after = if actions.is_empty() {
        first
    } else {
        nottools::run(&cfg, dotfiles_dir, home, fs, probe)?
    };
    for m in after.missing {
        match m.declared_in {
            Some(_) => summary.unresolved.push(m.target),
            None => summary.undeclared.push(m.target),
        }
    }
    summary.unresolved.sort();
    summary.unresolved.dedup();
    Ok(Some(summary))
}

/// [`reconcile`] against the real machine.
pub fn reconcile_system(dotfiles_dir: &Path) -> Result<Option<ToolsSummary>> {
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")?;
    reconcile(
        dotfiles_dir,
        &home,
        &SystemFs,
        &SystemProbe,
        &SystemInstaller,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nottools::adapters::{FakeProbe, MemoryFs, RecordingInstaller};

    const NOTFILES: &str = "[doctor]\nmise = \"/d/mise.toml\"\nbrewfiles = [\"/d/Brewfile\"]\nalias_files = [\"/h/.zshrc\"]\n";

    fn fs() -> MemoryFs {
        MemoryFs::new(&[
            ("/d/notfiles.toml", NOTFILES),
            ("/d/mise.toml", "[tools]\npyroclear = \"1\"\n"),
            ("/d/Brewfile", "brew \"colima\"\n"),
            (
                "/h/.zshrc",
                "alias a='pyroclear'\nalias b='colima x'\nalias c='zb y'\n",
            ),
        ])
    }

    #[test]
    fn no_doctor_table_is_none() {
        let fs = MemoryFs::new(&[("/d/notfiles.toml", "[defaults]\ntarget = \"~\"\n")]);
        let probe = FakeProbe::new(&[], &[], vec![]);
        let got = reconcile(
            Path::new("/d"),
            Path::new("/h"),
            &fs,
            &probe,
            &RecordingInstaller::new(true),
        );
        assert_eq!(got.unwrap(), None);
    }

    #[test]
    fn installs_declared_and_reports_undeclared() {
        let probe = FakeProbe::new(&[], &[], vec![]);
        let inst = RecordingInstaller::new(true);
        let got = reconcile(Path::new("/d"), Path::new("/h"), &fs(), &probe, &inst)
            .unwrap()
            .unwrap();
        assert_eq!(inst.calls(), ["mise install", "brew install colima"]);
        assert_eq!(got.installed, ["mise install", "brew install colima"]);
        assert_eq!(got.undeclared, ["zb"]);
        // The fake probe never gains the binaries, so the re-check still sees them missing.
        assert_eq!(got.unresolved, ["colima", "pyroclear"]);
    }

    #[test]
    fn failed_install_is_unresolved() {
        let probe = FakeProbe::new(&[], &[], vec![]);
        let inst = RecordingInstaller::new(false);
        let got = reconcile(Path::new("/d"), Path::new("/h"), &fs(), &probe, &inst)
            .unwrap()
            .unwrap();
        assert!(got.installed.is_empty());
        assert_eq!(
            got.unresolved,
            ["brew install colima", "colima", "mise install", "pyroclear"]
        );
        assert_eq!(got.undeclared, ["zb"]);
    }

    #[test]
    fn nothing_missing_installs_nothing() {
        let probe = FakeProbe::new(&["pyroclear", "colima", "zb"], &[], vec![]);
        let inst = RecordingInstaller::new(true);
        let got = reconcile(Path::new("/d"), Path::new("/h"), &fs(), &probe, &inst)
            .unwrap()
            .unwrap();
        assert_eq!(got, ToolsSummary::default());
        assert!(inst.calls().is_empty());
    }
}
