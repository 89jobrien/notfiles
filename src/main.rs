mod cli;
mod config;
mod error;
mod ignore;
mod linker;
mod package;
mod paths;
mod status;

use std::fs;

use anyhow::{Context, Result};
use clap::Parser;

use cli::{Cli, Command};
use config::Config;
use linker::{LinkOptions, State};
use package::resolve_packages;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let dotfiles_dir = cli
        .dir
        .unwrap_or_else(|| std::env::current_dir().expect("cannot determine current directory"));
    let dotfiles_dir = fs::canonicalize(&dotfiles_dir)
        .with_context(|| format!("dotfiles directory not found: {}", dotfiles_dir.display()))?;

    match cli.command {
        Command::Init => cmd_init(&dotfiles_dir)?,
        Command::Link {
            force,
            no_backup,
            packages,
        } => {
            let config = Config::load(&dotfiles_dir)?;
            let mut state = State::load(&dotfiles_dir)?;
            let pkgs = resolve_packages(&dotfiles_dir, &packages)?;
            let opts = LinkOptions {
                force,
                no_backup,
                dry_run: cli.dry_run,
                verbose: cli.verbose,
            };

            if cli.dry_run {
                println!("\x1b[36m(dry run)\x1b[0m");
            }

            for pkg in &pkgs {
                if cli.verbose || cli.dry_run {
                    println!("Linking {pkg}...");
                }
                linker::link_package(&dotfiles_dir, &config, &mut state, pkg, &opts)?;
            }

            if !cli.dry_run {
                state.save(&dotfiles_dir)?;
                let count: usize = pkgs.iter().map(|p| state.entries_for_package(p).len()).sum();
                println!(
                    "\x1b[32mLinked {count} file{} across {} package{}.\x1b[0m",
                    if count == 1 { "" } else { "s" },
                    pkgs.len(),
                    if pkgs.len() == 1 { "" } else { "s" },
                );
            }
        }
        Command::Unlink { packages } => {
            let config = Config::load(&dotfiles_dir)?;
            let mut state = State::load(&dotfiles_dir)?;
            let pkgs = if packages.is_empty() {
                state
                    .entries
                    .iter()
                    .map(|e| e.package.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
            } else {
                // Validate requested packages exist in state
                let _ = resolve_packages(&dotfiles_dir, &packages).or_else(|_| {
                    // Package dir might be gone but state entries exist — that's fine for unlink
                    Ok::<Vec<String>, anyhow::Error>(packages.clone())
                });
                packages
            };
            let opts = LinkOptions {
                force: false,
                no_backup: false,
                dry_run: cli.dry_run,
                verbose: cli.verbose,
            };

            if cli.dry_run {
                println!("\x1b[36m(dry run)\x1b[0m");
            }

            let _ = &config; // loaded but not needed for unlink
            for pkg in &pkgs {
                if cli.verbose || cli.dry_run {
                    println!("Unlinking {pkg}...");
                }
                linker::unlink_package(&dotfiles_dir, &mut state, pkg, &opts)?;
            }

            if !cli.dry_run {
                state.save(&dotfiles_dir)?;
                println!("\x1b[32mUnlinked {} package{}.\x1b[0m", pkgs.len(), if pkgs.len() == 1 { "" } else { "s" });
            }
        }
        Command::Status { packages } => {
            let config = Config::load(&dotfiles_dir)?;
            let state = State::load(&dotfiles_dir)?;
            let pkgs = resolve_packages(&dotfiles_dir, &packages)?;

            for pkg in &pkgs {
                let entries = status::package_status(&dotfiles_dir, &config, &state, pkg);
                status::print_status(pkg, &entries);
            }
        }
    }

    Ok(())
}

fn cmd_init(dotfiles_dir: &std::path::Path) -> Result<()> {
    let config_path = dotfiles_dir.join("notfiles.toml");
    if config_path.exists() {
        println!("notfiles.toml already exists.");
        return Ok(());
    }
    fs::write(&config_path, config::starter_toml())?;
    println!("Created notfiles.toml");
    Ok(())
}
