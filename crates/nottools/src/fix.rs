use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::doctor::{Backend, DoctorReport};
use crate::ports::{Fs, InstalledCrate, Installer};

/// One reconciling step, as planned or performed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum FixAction {
    MiseInstall,
    BrewInstall { formula: String },
    Adopt { name: String, version: String },
}

impl FixAction {
    pub fn describe(&self) -> String {
        match self {
            Self::MiseInstall => "mise install".to_string(),
            Self::BrewInstall { formula } => format!("brew install {formula}"),
            Self::Adopt { name, version } => format!("declare cargo:{name} = {version}"),
        }
    }
}

/// Install declared-but-missing tools. With `dry_run` nothing runs and every action
/// is reported as succeeded.
pub fn apply_fixes(
    report: &DoctorReport,
    installer: &dyn Installer,
    dry_run: bool,
) -> Vec<(FixAction, bool)> {
    let mut plan = Vec::new();
    if report
        .missing
        .iter()
        .any(|m| m.declared_in == Some(Backend::Mise))
    {
        plan.push(FixAction::MiseInstall);
    }
    for m in report
        .missing
        .iter()
        .filter(|m| m.declared_in == Some(Backend::Brew))
    {
        plan.push(FixAction::BrewInstall {
            formula: m.target.clone(),
        });
    }
    plan.into_iter()
        .map(|action| {
            let ok = dry_run
                || match &action {
                    FixAction::MiseInstall => installer.mise_install().unwrap_or(false),
                    FixAction::BrewInstall { formula } => {
                        installer.brew_install(formula).unwrap_or(false)
                    }
                    FixAction::Adopt { .. } => true,
                };
            (action, ok)
        })
        .collect()
}

/// Declare crates.io crates in the mise config, after the last `"cargo:` entry
/// (or directly under `[tools]` when there is none).
pub fn adopt(
    mise_path: &Path,
    crates: &[InstalledCrate],
    fs: &dyn Fs,
    dry_run: bool,
) -> Result<Vec<FixAction>> {
    let actions: Vec<FixAction> = crates
        .iter()
        .map(|c| FixAction::Adopt {
            name: c.name.clone(),
            version: c.version.clone(),
        })
        .collect();
    if dry_run || crates.is_empty() {
        return Ok(actions);
    }
    let text = fs
        .read_to_string(mise_path)
        .with_context(|| format!("reading {}", mise_path.display()))?;
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let at = lines
        .iter()
        .rposition(|l| l.trim().starts_with("\"cargo:"))
        .or_else(|| lines.iter().position(|l| l.trim() == "[tools]"));
    let Some(at) = at else {
        bail!("{} has no [tools] table to adopt into", mise_path.display());
    };
    for (i, c) in crates.iter().enumerate() {
        lines.insert(
            at + 1 + i,
            format!("\"cargo:{}\" = \"{}\"", c.name, c.version),
        );
    }
    fs.write(mise_path, &(lines.join("\n") + "\n"))
        .with_context(|| format!("writing {}", mise_path.display()))?;
    Ok(actions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{MemoryFs, RecordingInstaller};
    use crate::doctor::MissingTool;
    use crate::ports::Source;

    fn missing(target: &str, declared_in: Option<Backend>) -> MissingTool {
        MissingTool {
            target: target.into(),
            alias: "a".into(),
            source: "s".into(),
            declared_in,
        }
    }

    fn krate(name: &str) -> InstalledCrate {
        InstalledCrate {
            name: name.into(),
            version: "1.2.3".into(),
            source: Source::Registry,
        }
    }

    #[test]
    fn installs_each_backend_once() {
        let report = DoctorReport {
            missing: vec![
                missing("a", Some(Backend::Mise)),
                missing("b", Some(Backend::Mise)),
                missing("colima", Some(Backend::Brew)),
                missing("zb", None),
            ],
            ..Default::default()
        };
        let inst = RecordingInstaller::new(true);
        let done = apply_fixes(&report, &inst, false);
        assert_eq!(inst.calls(), ["mise install", "brew install colima"]);
        assert!(done.iter().all(|(_, ok)| *ok));
    }

    #[test]
    fn dry_run_runs_nothing() {
        let report = DoctorReport {
            missing: vec![missing("a", Some(Backend::Mise))],
            ..Default::default()
        };
        let inst = RecordingInstaller::new(true);
        let done = apply_fixes(&report, &inst, true);
        assert!(inst.calls().is_empty());
        assert_eq!(done.len(), 1);
    }

    #[test]
    fn failed_install_is_reported() {
        let report = DoctorReport {
            missing: vec![missing("a", Some(Backend::Mise))],
            ..Default::default()
        };
        let done = apply_fixes(&report, &RecordingInstaller::new(false), false);
        assert!(!done[0].1);
    }

    #[test]
    fn adopt_inserts_after_last_cargo_entry() {
        let fs = MemoryFs::new(&[(
            "/m.toml",
            "[tools]\n\"cargo:nu\" = \"1\"\nnode = \"20\"\n\"cargo:just\" = \"2\"\nother = \"3\"\n",
        )]);
        adopt(Path::new("/m.toml"), &[krate("bacon")], &fs, false).unwrap();
        let out = fs.get("/m.toml").unwrap();
        assert!(out.contains("\"cargo:just\" = \"2\"\n\"cargo:bacon\" = \"1.2.3\"\nother"));
    }

    #[test]
    fn adopt_falls_back_to_tools_table() {
        let fs = MemoryFs::new(&[("/m.toml", "[tools]\nnode = \"20\"\n")]);
        adopt(Path::new("/m.toml"), &[krate("bacon")], &fs, false).unwrap();
        assert!(
            fs.get("/m.toml")
                .unwrap()
                .starts_with("[tools]\n\"cargo:bacon\"")
        );
    }

    #[test]
    fn adopt_dry_run_does_not_write() {
        let fs = MemoryFs::new(&[("/m.toml", "[tools]\n")]);
        let acts = adopt(Path::new("/m.toml"), &[krate("bacon")], &fs, true).unwrap();
        assert_eq!(acts.len(), 1);
        assert_eq!(fs.get("/m.toml").unwrap(), "[tools]\n");
    }

    #[test]
    fn adopt_without_tools_table_errors() {
        let fs = MemoryFs::new(&[("/m.toml", "[env]\nA = \"1\"\n")]);
        assert!(adopt(Path::new("/m.toml"), &[krate("bacon")], &fs, false).is_err());
    }
}
