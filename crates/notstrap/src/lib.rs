use anyhow::{Context, Result};
use notcore::{HookPhase, Report, StepStatus};
use notfiles::{link, LinkOptions};
use nothooks::{run_phase, HookRunner};
use notsecrets::{install_age_key, resolve_age_key, BitwardenSource, FileSource, PromptSource};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub mod prereqs;
pub mod repo;

type EnvInjector = Box<dyn Fn(&Path) -> Result<String>>;

#[derive(Deserialize)]
pub struct NotstrapConfig {
    pub bootstrap: BootstrapSection,
    #[serde(default)]
    pub hooks: Vec<notcore::HookSpec>,
}

#[derive(Deserialize)]
pub struct BootstrapSection {
    pub dotfiles_repo: String,
    pub dotfiles_dir: String,
    #[serde(default = "default_bw_item")]
    pub bw_age_item: String,
    #[serde(default = "default_sops_file")]
    pub sops_file: String,
}

pub(crate) fn default_bw_item() -> String { "age-key-dotfiles".to_string() }
pub(crate) fn default_sops_file() -> String { "secrets/bootstrap.sops.env".to_string() }

pub struct BootstrapOptions {
    pub config: PathBuf,
    pub force: bool,
    pub key_file: Option<PathBuf>,
    pub dotfiles: Option<PathBuf>,
    /// None = skip prereq check (tests). Some(f) = run f().
    pub check_prereqs: Option<Box<dyn Fn() -> Result<()>>>,
    /// None = skip env injection (tests). Some(f) = decrypt sops at path and inject.
    pub env_injector: Option<EnvInjector>,
}

pub fn run(opts: BootstrapOptions) -> Result<Report> {
    let mut report = Report::default();

    // 1. Prerequisites
    if let Some(check) = opts.check_prereqs {
        match check() {
            Ok(_) => { report.add("prerequisites", StepStatus::Ok); }
            Err(e) => {
                report.add("prerequisites", StepStatus::Failed(e.to_string()));
                return Ok(report);
            }
        }
    }

    // 2. Load config
    let config_content = std::fs::read_to_string(&opts.config)
        .with_context(|| format!("cannot read {}", opts.config.display()))?;
    let cfg: NotstrapConfig = toml::from_str(&config_content)?;

    let dotfiles_dir = match opts.dotfiles {
        Some(d) => d,
        None => notcore::expand_tilde(&cfg.bootstrap.dotfiles_dir)
            .with_context(|| format!("cannot expand dotfiles_dir: {}", cfg.bootstrap.dotfiles_dir))?,
    };

    // 3. Clone dotfiles if missing
    match repo::clone_if_missing(&cfg.bootstrap.dotfiles_repo, &dotfiles_dir) {
        Ok(true)  => { report.add("clone dotfiles", StepStatus::Ok); }
        Ok(false) => { report.add("clone dotfiles", StepStatus::Skipped); }
        Err(e)    => {
            report.add("clone dotfiles", StepStatus::Failed(e.to_string()));
            return Ok(report);
        }
    }

    // 4. Retrieve age key and install
    let sources: Vec<Box<dyn notsecrets::AgeKeySource>> = if let Some(kf) = opts.key_file {
        vec![Box::new(FileSource::new(kf))]
    } else {
        vec![
            Box::new(BitwardenSource::new(&cfg.bootstrap.bw_age_item)),
            Box::new(PromptSource),
        ]
    };

    match resolve_age_key(sources) {
        Ok(key) => {
            install_age_key(&key)?;
            report.add("age key", StepStatus::Ok);
        }
        Err(e) => {
            report.add("age key", StepStatus::Failed(e.to_string()));
            return Ok(report);
        }
    }

    // 5. Decrypt sops secrets (optional)
    if let Some(injector) = opts.env_injector {
        let sops_path = dotfiles_dir.join(&cfg.bootstrap.sops_file);
        match injector(&sops_path) {
            Ok(env_content) => {
                for line in env_content.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        let k = k.trim();
                        let v = v.trim().trim_matches('"');
                        if !k.is_empty() && !k.starts_with('#') {
                            // Safety: single-threaded bootstrap, no concurrent env readers
                            unsafe { std::env::set_var(k, v); }
                        }
                    }
                }
                report.add("decrypt secrets", StepStatus::Ok);
            }
            Err(e) => {
                report.add("decrypt secrets", StepStatus::Failed(e.to_string()));
                return Ok(report);
            }
        }
    }

    // 6. Link dotfiles
    let link_opts = LinkOptions { force: opts.force, no_backup: false, dry_run: false, verbose: false };
    match link(&dotfiles_dir, &[], &link_opts) {
        Ok(state) => {
            let count = state.entries.len();
            report.add(format!("link dotfiles ({count} files)"), StepStatus::Ok);
        }
        Err(e) => {
            report.add("link dotfiles", StepStatus::Failed(e.to_string()));
        }
    }

    // 7. Run hooks
    let runner = if opts.force {
        HookRunner::with_force(dotfiles_dir.clone())
    } else {
        HookRunner::new(dotfiles_dir.clone())
    };

    for (phase, label) in [(HookPhase::Dot, "dot hooks"), (HookPhase::Setup, "setup hooks")] {
        let phase_report = run_phase(&cfg.hooks, &phase, &runner);
        let failed = phase_report.steps.iter()
            .filter(|s| matches!(s.status, notcore::StepStatus::Failed(_)))
            .count();
        let summary = if failed > 0 {
            StepStatus::Failed(format!("{failed} failed"))
        } else {
            StepStatus::Ok
        };
        report.add(label, summary);
    }

    Ok(report)
}
