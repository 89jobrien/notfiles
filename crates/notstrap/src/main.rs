use anyhow::Result;
use clap::Parser;
use notstrap::prompt::{LinePrompter, Prompter};
use notstrap::{BootstrapOptions, prereqs, run, tools};
use std::io::IsTerminal;

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
        config: std::path::PathBuf,

        /// Force re-run of setup hooks
        #[arg(long)]
        force: bool,

        /// Path to age key file (skips Bitwarden and prompt)
        #[arg(long)]
        key_file: Option<std::path::PathBuf>,

        /// Path to dotfiles directory (default: ~/dotfiles)
        #[arg(long)]
        dotfiles: Option<std::path::PathBuf>,

        /// Accept every default without prompting: skips the wizard, and tells hooks not to
        /// prompt (`NOTSTRAP_YES=1`). Without it, a terminal session runs the wizard.
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let Cmd::Run {
        config,
        force,
        key_file,
        dotfiles,
        yes,
    } = cli.command;
    // No prompts without a terminal either: bootstrap stays scriptable.
    let interactive = !yes && std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let prompter: Option<Box<dyn Prompter>> = if interactive {
        Some(Box::new(LinePrompter::stdio()))
    } else {
        None
    };
    let opts = BootstrapOptions {
        config,
        force,
        key_file,
        dotfiles,
        tailscale: None, // defer to [tailscale] section in config (if present)
        check_prereqs: Some(Box::new(prereqs::check_prerequisites)),
        reconcile_tools: Some(Box::new(tools::reconcile_system)),
        git: Box::new(notstrap::repo::SystemGit),
        prompter,
        assume_yes: yes,
        secrets_config: None,
    };
    let report = run(opts)?;
    report.print();
    if report.has_failures() {
        std::process::exit(1);
    }
    Ok(())
}
