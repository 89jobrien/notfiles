use anyhow::{Context, Result};
use clap::Parser;
use notcore::{HookPhase, Report, StepStatus};
use notfiles::{link, LinkOptions};
use nothooks::{run_phase, HookRunner};
use notsecrets::{
    install_age_key, resolve_age_key, BitwardenSource, FileSource, PromptSource,
};
use serde::Deserialize;
use std::path::PathBuf;

mod prereqs;
mod repo;

#[derive(Parser)]
#[command(name = "notstrap", about = "Bootstrap a new machine from dotfiles")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(clap::Subcommand)]
enum Cmd {
    /// Run the full bootstrap sequence
    Run {
        /// Path to notstrap.toml config
        #[arg(long, default_value = "notstrap.toml")]
        config: PathBuf,

        /// Force re-run of setup hooks
        #[arg(long)]
        force: bool,

        /// Path to age key file (skips Bitwarden and prompt)
        #[arg(long)]
        key_file: Option<PathBuf>,

        /// Path to dotfiles directory (default: ~/dotfiles)
        #[arg(long)]
        dotfiles: Option<PathBuf>,
    },
}

#[derive(Deserialize)]
struct NotstrapConfig {
    bootstrap: BootstrapSection,
    #[serde(default)]
    hooks: Vec<notcore::HookSpec>,
}

#[derive(Deserialize)]
struct BootstrapSection {
    dotfiles_repo: String,
    dotfiles_dir: String,
    #[serde(default = "default_bw_item")]
    bw_age_item: String,
    #[serde(default = "default_sops_file")]
    sops_file: String,
}

fn default_bw_item() -> String { "age-key-dotfiles".to_string() }
fn default_sops_file() -> String { "secrets/bootstrap.sops.env".to_string() }

fn main() -> Result<()> {
    let cli = Cli::parse();
    let Cmd::Run { config, force, key_file, dotfiles } = cli.command;

    let mut report = Report::default();

    // 1. Prerequisites
    print!("Checking prerequisites... ");
    match prereqs::check_prerequisites() {
        Ok(_) => { println!("ok"); report.add("prerequisites", StepStatus::Ok); }
        Err(e) => {
            println!("FAILED");
            report.add("prerequisites", StepStatus::Failed(e.to_string()));
            report.print();
            std::process::exit(1);
        }
    }

    // 2. Load config
    let config_content = std::fs::read_to_string(&config)
        .with_context(|| format!("cannot read {}", config.display()))?;
    let cfg: NotstrapConfig = toml::from_str(&config_content)?;

    let dotfiles_dir = dotfiles.unwrap_or_else(|| {
        notcore::expand_tilde(&cfg.bootstrap.dotfiles_dir).unwrap()
    });

    // 3. Clone dotfiles if missing
    match repo::clone_if_missing(&cfg.bootstrap.dotfiles_repo, &dotfiles_dir) {
        Ok(true)  => { println!("Cloned dotfiles."); report.add("clone dotfiles", StepStatus::Ok); }
        Ok(false) => { report.add("clone dotfiles", StepStatus::Skipped); }
        Err(e)    => {
            report.add("clone dotfiles", StepStatus::Failed(e.to_string()));
            report.print();
            std::process::exit(1);
        }
    }

    // 4. Retrieve age key and install
    print!("Retrieving age key... ");
    let sources: Vec<Box<dyn notsecrets::AgeKeySource>> = if let Some(kf) = key_file {
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
            println!("ok");
            report.add("age key", StepStatus::Ok);
        }
        Err(e) => {
            println!("FAILED");
            report.add("age key", StepStatus::Failed(e.to_string()));
            report.print();
            std::process::exit(1);
        }
    }

    // 5. Decrypt sops secrets
    let sops_path = dotfiles_dir.join(&cfg.bootstrap.sops_file);
    match notsecrets::decrypt_sops(&sops_path) {
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
            report.print();
            std::process::exit(1);
        }
    }

    // 6. Link dotfiles
    let opts = LinkOptions { force: false, no_backup: false, dry_run: false, verbose: false };
    match link(&dotfiles_dir, &[], &opts) {
        Ok(state) => {
            let count = state.entries.len();
            println!("Linked {count} files.");
            report.add(format!("link dotfiles ({count} files)"), StepStatus::Ok);
        }
        Err(e) => {
            report.add("link dotfiles", StepStatus::Failed(e.to_string()));
        }
    }

    // 7. Run hooks
    let runner = if force {
        HookRunner::with_force(dotfiles_dir.clone())
    } else {
        HookRunner::new(dotfiles_dir.clone())
    };

    for (phase, label) in [(HookPhase::Dot, "dot hooks"), (HookPhase::Setup, "setup hooks")] {
        let phase_report = run_phase(&cfg.hooks, &phase, &runner);
        let failed = phase_report.steps.iter().filter(|s| matches!(s.status, notcore::StepStatus::Failed(_))).count();
        let summary = if failed > 0 {
            StepStatus::Failed(format!("{failed} failed"))
        } else {
            StepStatus::Ok
        };
        report.add(label, summary);
        phase_report.print();
    }

    // 8. Final report
    println!("\n── Bootstrap complete ──");
    report.print();

    if report.has_failures() {
        std::process::exit(1);
    }
    Ok(())
}
