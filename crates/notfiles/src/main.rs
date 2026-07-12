use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use std::fs;

use notcore::Config;
use notcore::reporter::Reporter;
use notfiles::adapters::{JsonReporter, TerminalReporter};
use notfiles::cli::{Cli, Command};
use notfiles::detect;
use notfiles::linker::{LinkOptions, State};
use notfiles::package::{resolve_packages_filtered_with_store, resolve_packages_with_store};
use notfiles::{adapters, linker, status};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let dotfiles_dir = cli
        .dir
        .unwrap_or_else(|| std::env::current_dir().expect("cannot determine current directory"));
    let dotfiles_dir = fs::canonicalize(&dotfiles_dir)
        .with_context(|| format!("dotfiles directory not found: {}", dotfiles_dir.display()))?;

    let reporter: Box<dyn Reporter> = if cli.json {
        Box::new(JsonReporter)
    } else {
        Box::new(TerminalReporter)
    };

    match cli.command {
        Command::Detect => {
            let home = std::env::var("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| dotfiles_dir.clone());
            let managers = detect::detect(&home);
            if cli.json {
                detect::print_detected_json(&managers);
            } else {
                detect::print_detected(&managers);
            }
        }
        Command::Init => cmd_init(&dotfiles_dir)?,
        Command::Link {
            force,
            no_backup,
            packages,
        } => {
            let fs = &adapters::FileStoreImpl;
            let config = Config::load(&dotfiles_dir)?;
            config.validate()?;
            let mut state = State::load(&dotfiles_dir, fs)?;
            let pkgs = resolve_packages_filtered_with_store(&dotfiles_dir, &packages, &config, fs)?;
            let opts = LinkOptions {
                force,
                no_backup,
                dry_run: cli.dry_run,
                verbose: cli.verbose,
            };

            if cli.dry_run {
                println!("\x1b[36m(dry run)\x1b[0m");
            }

            let mut results = Vec::new();
            for pkg in &pkgs {
                if cli.verbose || cli.dry_run {
                    println!("Linking {pkg}...");
                }
                let result = linker::link_package(
                    &dotfiles_dir,
                    &config,
                    &mut state,
                    pkg,
                    &opts,
                    fs,
                    &*reporter,
                )?;
                results.push(result);
            }

            if !cli.dry_run {
                state.save(&dotfiles_dir, fs)?;
                let count: usize = results.iter().map(|r| r.linked + r.copied).sum();
                println!(
                    "\x1b[32mLinked {count} file{} across {} package{}.\x1b[0m",
                    if count == 1 { "" } else { "s" },
                    pkgs.len(),
                    if pkgs.len() == 1 { "" } else { "s" },
                );
            }
        }
        Command::Unlink { packages } => {
            let fs = &adapters::FileStoreImpl;
            let config = Config::load(&dotfiles_dir)?;
            let _ = &config; // loaded but not needed for unlink
            let mut state = State::load(&dotfiles_dir, fs)?;
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
                let _ = resolve_packages_with_store(&dotfiles_dir, &packages, fs).or_else(|_| {
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

            for pkg in &pkgs {
                if cli.verbose || cli.dry_run {
                    println!("Unlinking {pkg}...");
                }
                linker::unlink_package(&dotfiles_dir, &mut state, pkg, &opts, fs, &*reporter)?;
            }

            if !cli.dry_run {
                state.save(&dotfiles_dir, fs)?;
                println!(
                    "\x1b[32mUnlinked {} package{}.\x1b[0m",
                    pkgs.len(),
                    if pkgs.len() == 1 { "" } else { "s" }
                );
            }
        }
        Command::Completions { shell } => {
            clap_complete::generate(
                shell,
                &mut Cli::command(),
                "notfiles",
                &mut std::io::stdout(),
            );
        }
        Command::Check => {
            let fs = &adapters::FileStoreImpl;
            let config = Config::load(&dotfiles_dir)?;
            config.validate()?;
            let all = resolve_packages_filtered_with_store(&dotfiles_dir, &[], &config, fs)?;

            if cli.json {
                let skipped: Vec<String> = {
                    let all_unfiltered =
                        notfiles::package::resolve_packages_with_store(&dotfiles_dir, &[], fs)?;
                    all_unfiltered
                        .into_iter()
                        .filter(|p| !all.contains(p))
                        .collect()
                };
                let obj = serde_json::json!({
                    "valid": true,
                    "packages": all,
                    "skipped": skipped,
                });
                println!("{}", serde_json::to_string(&obj).unwrap_or_default());
            } else {
                println!("notfiles.toml: \x1b[32mvalid\x1b[0m");
                println!("Packages: {} ({} total)", all.join(", "), all.len());
            }
        }
        Command::Diff { packages } => {
            let fs = &adapters::FileStoreImpl;
            let config = Config::load(&dotfiles_dir)?;
            config.validate()?;
            let state = State::load(&dotfiles_dir, fs)?;
            let pkgs = resolve_packages_filtered_with_store(&dotfiles_dir, &packages, &config, fs)?;

            for pkg in &pkgs {
                let entries = status::diff_package(&dotfiles_dir, &config, &state, pkg, fs);
                status::print_diff(pkg, &entries);
            }
        }
        Command::Adopt { package, files } => {
            let fs = &adapters::FileStoreImpl;
            let config = Config::load(&dotfiles_dir)?;
            let mut state = State::load(&dotfiles_dir, fs)?;
            let opts = LinkOptions {
                force: false,
                no_backup: false,
                dry_run: cli.dry_run,
                verbose: cli.verbose,
            };

            if cli.dry_run {
                println!("\x1b[36m(dry run)\x1b[0m");
            }

            let result = linker::adopt_files(
                &dotfiles_dir,
                &config,
                &mut state,
                &package,
                &files,
                &opts,
                fs,
                &*reporter,
            )?;

            if !cli.dry_run {
                println!(
                    "\x1b[32mAdopted {} file{} into {package}.\x1b[0m",
                    result.linked,
                    if result.linked == 1 { "" } else { "s" },
                );
            }
        }
        Command::Status { packages } => {
            let fs = &adapters::FileStoreImpl;
            let config = Config::load(&dotfiles_dir)?;
            config.validate()?;
            let state = State::load(&dotfiles_dir, fs)?;
            let pkgs = resolve_packages_filtered_with_store(&dotfiles_dir, &packages, &config, fs)?;

            for pkg in &pkgs {
                let entries = status::package_status(&dotfiles_dir, &config, &state, pkg, fs);
                if cli.json {
                    status::print_status_json(pkg, &entries);
                } else {
                    status::print_status(pkg, &entries);
                }
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
    fs::write(&config_path, notcore::config::starter_toml())?;
    println!("Created notfiles.toml");
    Ok(())
}
