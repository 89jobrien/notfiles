use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::aliases::{self, Alias};
use crate::config::DoctorConfig;
use crate::declared::{brew_names, mise_tools};
use crate::ports::{Fs, InstalledCrate, Source, ToolProbe};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Mise,
    Brew,
}

/// An alias target that does not resolve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MissingTool {
    pub target: String,
    pub alias: String,
    pub source: String,
    /// Where the tool is declared, if anywhere; decides the install command.
    pub declared_in: Option<Backend>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub missing: Vec<MissingTool>,
    /// crates.io crates installed with `cargo install` but declared nowhere.
    pub undeclared: Vec<InstalledCrate>,
    /// Git-sourced crates declared nowhere.
    pub git: Vec<InstalledCrate>,
    /// Path-built crates declared nowhere; not reproducible from a manifest.
    pub local: Vec<String>,
}

impl DoctorReport {
    /// Missing alias targets fail the check; undeclared crates are advisory.
    pub fn has_failures(&self) -> bool {
        !self.missing.is_empty()
    }

    pub fn render(&self) -> String {
        let mut out = String::from("== Alias targets\n");
        if self.missing.is_empty() {
            out.push_str("  all alias targets resolve\n");
        }
        for m in &self.missing {
            let hint = match m.declared_in {
                Some(Backend::Mise) => "declared in mise, run `mise install`".to_string(),
                Some(Backend::Brew) => {
                    format!("declared in a Brewfile, run `brew install {}`", m.target)
                }
                None => "not declared anywhere".to_string(),
            };
            let _ = writeln!(
                out,
                "  MISSING  {} (alias {} in {}): {hint}",
                m.target, m.alias, m.source
            );
        }
        out.push_str("== Cargo crates not declared in mise or a Brewfile\n");
        for c in &self.undeclared {
            let _ = writeln!(out, "  UNDECLARED  {} {}", c.name, c.version);
        }
        if self.undeclared.is_empty() {
            out.push_str("  none\n");
        }
        if !self.git.is_empty() {
            out.push_str(
                "== Git-sourced (pin with `\"cargo:<url>\" = \"branch:<name>\"` in mise)\n",
            );
            for c in &self.git {
                if let Source::Git(url) = &c.source {
                    let _ = writeln!(out, "  GIT  {}  {url}", c.name);
                }
            }
        }
        if !self.local.is_empty() {
            let _ = writeln!(
                out,
                "== Built from a local path (not reproducible via mise): {}",
                self.local.join(", ")
            );
        }
        out
    }
}

/// Expand a leading `~/` against `home`; everything else is used as written.
fn target_path(target: &str, home: &Path) -> Option<PathBuf> {
    if let Some(rest) = target.strip_prefix("~/") {
        return Some(home.join(rest));
    }
    target.contains('/').then(|| PathBuf::from(target))
}

fn resolves(target: &str, home: &Path, probe: &dyn ToolProbe) -> bool {
    match target_path(target, home) {
        Some(p) => probe.path_exists(&p),
        None => probe.on_path(target),
    }
}

/// Files to scan: each configured entry, expanded one directory level.
fn expand_alias_files(entries: &[PathBuf], fs: &dyn Fs) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for e in entries {
        match fs.list_dir(e) {
            Ok(children) => files.extend(children),
            Err(_) => files.push(e.clone()),
        }
    }
    files
}

pub fn run(
    cfg: &DoctorConfig,
    dotfiles_dir: &Path,
    home: &Path,
    fs: &dyn Fs,
    probe: &dyn ToolProbe,
) -> Result<DoctorReport> {
    let mut in_mise = BTreeSet::new();
    if let Some(p) = cfg.mise_path(dotfiles_dir)? {
        let text = fs
            .read_to_string(&p)
            .with_context(|| format!("reading {}", p.display()))?;
        in_mise = mise_tools(&text);
    }
    let mut in_brew = BTreeSet::new();
    for p in cfg.brewfile_paths(dotfiles_dir)? {
        let text = fs
            .read_to_string(&p)
            .with_context(|| format!("reading {}", p.display()))?;
        in_brew.extend(brew_names(&text));
    }

    let mut found: Vec<Alias> = Vec::new();
    let mut functions = BTreeSet::new();
    for f in expand_alias_files(&cfg.alias_paths(dotfiles_dir)?, fs) {
        let Ok(text) = fs.read_to_string(&f) else {
            continue;
        };
        let scan = aliases::scan(&text, &f.display().to_string());
        found.extend(scan.aliases);
        functions.extend(scan.functions);
    }
    let defined: BTreeSet<&str> = found
        .iter()
        .map(|a| a.name.as_str())
        .chain(functions.iter().map(String::as_str))
        .collect();

    let mut report = DoctorReport::default();
    let mut seen = BTreeSet::new();
    for a in found
        .iter()
        .filter(|a| !a.guarded && aliases::is_checkable(&a.target))
    {
        if defined.contains(a.target.as_str())
            || resolves(&a.target, home, probe)
            || !seen.insert(a.target.as_str())
        {
            continue;
        }
        let declared_in = if in_mise.contains(&a.target) {
            Some(Backend::Mise)
        } else if in_brew.contains(&a.target) {
            Some(Backend::Brew)
        } else {
            None
        };
        report.missing.push(MissingTool {
            target: a.target.clone(),
            alias: a.name.clone(),
            source: a.source.clone(),
            declared_in,
        });
    }

    for c in probe.installed_crates() {
        if in_mise.contains(&c.name) || in_brew.contains(&c.name) {
            continue;
        }
        match c.source {
            Source::Registry => report.undeclared.push(c),
            Source::Git(_) => report.git.push(c),
            Source::Path => report.local.push(c.name),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{FakeProbe, MemoryFs};

    fn cfg() -> DoctorConfig {
        DoctorConfig {
            mise: Some("/m/config.toml".into()),
            brewfiles: vec!["/m/Brewfile".into()],
            alias_files: vec!["/h/.zshrc".into(), "/h/autoload".into()],
        }
    }

    fn fs() -> MemoryFs {
        MemoryFs::new(&[
            (
                "/m/config.toml",
                "[tools]\n\"cargo:pyroclear\" = \"1\"\njust = \"1\"\n",
            ),
            ("/m/Brewfile", "brew \"colima\"\n"),
            (
                "/h/.zshrc",
                "alias clear='pyroclear'\nalias cs='colima start'\nalias zbi='zb install'\nif command -v pj; then\n  alias o='pj x'\nfi\nalias j='just'\nalias t='~/bin/tool.sh'\n",
            ),
            (
                "/h/autoload/a.nu",
                "alias gg = git\nalias w = who\nexport def who [] {}\n",
            ),
        ])
    }

    #[test]
    fn classifies_missing_targets_by_declaration() {
        let probe = FakeProbe::new(&["git", "just"], &[], vec![]);
        let r = run(&cfg(), Path::new("/d"), Path::new("/home"), &fs(), &probe)
            .expect("run should succeed");
        let by: Vec<(&str, Option<Backend>)> = r
            .missing
            .iter()
            .map(|m| (m.target.as_str(), m.declared_in))
            .collect();
        assert!(by.contains(&("pyroclear", Some(Backend::Mise))));
        assert!(by.contains(&("colima", Some(Backend::Brew))));
        assert!(by.contains(&("zb", None)));
        assert!(by.contains(&("~/bin/tool.sh", None)));
        assert!(
            !by.iter().any(|(t, _)| *t == "pj"),
            "guarded alias must be skipped"
        );
        assert!(
            !by.iter().any(|(t, _)| *t == "who"),
            "function target is defined"
        );
        assert!(r.has_failures());
    }

    #[test]
    fn resolved_targets_pass() {
        let probe = FakeProbe::new(
            &["git", "just", "pyroclear", "colima", "zb"],
            &["/home/bin/tool.sh"],
            vec![],
        );
        let r = run(&cfg(), Path::new("/d"), Path::new("/home"), &fs(), &probe)
            .expect("run should succeed");
        assert!(r.missing.is_empty(), "{:?}", r.missing);
    }

    #[test]
    fn splits_installed_crates_by_source() {
        let crates = vec![
            InstalledCrate {
                name: "pyroclear".into(),
                version: "1".into(),
                source: Source::Registry,
            },
            InstalledCrate {
                name: "bacon".into(),
                version: "3".into(),
                source: Source::Registry,
            },
            InstalledCrate {
                name: "g".into(),
                version: "1".into(),
                source: Source::Git("https://x/g".into()),
            },
            InstalledCrate {
                name: "mine".into(),
                version: "1".into(),
                source: Source::Path,
            },
        ];
        let probe = FakeProbe::new(&[], &[], crates);
        let r = run(&cfg(), Path::new("/d"), Path::new("/home"), &fs(), &probe)
            .expect("run should succeed");
        assert_eq!(
            r.undeclared
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["bacon"]
        );
        assert_eq!(r.git.len(), 1);
        assert_eq!(r.local, ["mine"]);
    }

    #[test]
    fn unreadable_manifest_is_an_error() {
        let probe = FakeProbe::new(&[], &[], vec![]);
        let empty = MemoryFs::new(&[]);
        assert!(run(&cfg(), Path::new("/d"), Path::new("/home"), &empty, &probe).is_err());
    }
}
