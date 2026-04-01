use anyhow::Result;
use clap::Parser;
use notstrap::{prereqs, run, BootstrapOptions};

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
    let cli = Cli::parse();
    let Cmd::Run { config, force, key_file, dotfiles } = cli.command;
    let opts = BootstrapOptions {
        config,
        force,
        key_file,
        dotfiles,
        check_prereqs: Some(Box::new(prereqs::check_prerequisites)),
        env_injector: Some(Box::new(notsecrets::decrypt_sops)),
    };
    let report = run(opts)?;
    report.print();
    if report.has_failures() {
        std::process::exit(1);
    }
    Ok(())
}
