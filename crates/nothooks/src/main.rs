use anyhow::Result;
use clap::{Parser, Subcommand};
use nothooks::{run_phase, HookRunner};
use notcore::{HookPhase, HookSpec};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "nothooks", about = "Bootstrap hook runner")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,

    /// Force re-run of setup hooks
    #[arg(long, global = true)]
    force: bool,

    /// Path to hooks config TOML
    #[arg(long, global = true, default_value = "notstrap.toml")]
    config: PathBuf,

    /// Directory for state file (default: current dir)
    #[arg(long, global = true)]
    state_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run hooks for a phase
    Run {
        /// Phase to run: dot or setup
        #[arg(long)]
        phase: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let state_dir = cli
        .state_dir
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let content = std::fs::read_to_string(&cli.config)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", cli.config.display()))?;

    #[derive(serde::Deserialize)]
    struct HooksFile {
        hooks: Vec<HookSpec>,
    }
    let file: HooksFile = toml::from_str(&content)?;

    let phase = match cli.command {
        Cmd::Run { ref phase } => match phase.as_str() {
            "dot" => HookPhase::Dot,
            "setup" => HookPhase::Setup,
            other => anyhow::bail!("unknown phase '{other}', use dot or setup"),
        },
    };

    let runner = if cli.force {
        HookRunner::with_force(state_dir)
    } else {
        HookRunner::new(state_dir)
    };

    let report = run_phase(&file.hooks, &phase, &runner);
    report.print();

    if report.has_failures() {
        std::process::exit(1);
    }
    Ok(())
}
