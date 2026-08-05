use anyhow::Result;
use clap::{CommandFactory, Parser};
use notstrap::{BootstrapOptions, prereqs, run};

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
    },
}

fn main() -> Result<()> {
    if std::env::args().nth(1).as_deref() == Some("completions") {
        clap_complete::generate(
            clap_complete_nushell::Nushell,
            &mut Cli::command(),
            "notstrap",
            &mut std::io::stdout(),
        );
        return Ok(());
    }
    let cli = Cli::parse();
    let Cmd::Run {
        config,
        force,
        key_file,
        dotfiles,
    } = cli.command;
    let opts = BootstrapOptions {
        config,
        force,
        key_file,
        dotfiles,
        tailscale: None, // defer to [tailscale] section in config (if present)
        check_prereqs: Some(Box::new(prereqs::check_prerequisites)),
        secrets_config: None,
    };
    let report = run(opts)?;
    report.print();
    if report.has_failures() {
        std::process::exit(1);
    }
    Ok(())
}
