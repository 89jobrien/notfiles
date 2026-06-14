//! A modern dotfiles manager — pure Rust alternative to GNU Stow.
//!
//! Symlinks (or copies) files from organized "package" directories
//! into a target location (typically `~`). Supports include/exclude
//! filtering, platform gates, and multiple output formats.
//!
//! # Quick start
//!
//! ```no_run
//! use notfiles::{link, LinkOptions};
//! use std::path::Path;
//!
//! let opts = LinkOptions {
//!     force: false,
//!     no_backup: false,
//!     dry_run: false,
//!     verbose: true,
//! };
//! let state = link(Path::new("/home/user/dotfiles"), &[], &opts)
//!     .expect("link failed");
//! ```

pub mod adapters;
pub mod cli;
pub mod detect;
pub mod ignore;
pub mod linker;
pub mod package;
pub mod ports;
pub mod status;

use std::path::Path;

use notcore::NotfilesError;
use notcore::reporter::Reporter;

pub use adapters::FileStoreImpl;
pub use linker::{LinkOptions, LinkResult, State};
pub use package::{
    discover_packages_filtered, resolve_packages, resolve_packages_filtered_with_store,
    resolve_packages_with_store,
};
pub use ports::FileStore;

/// Link all (or specified) packages using the real filesystem and
/// terminal output.
pub fn link(
    dotfiles_dir: &Path,
    packages: &[String],
    opts: &LinkOptions,
) -> Result<State, NotfilesError> {
    let reporter = adapters::TerminalReporter;
    link_with_store(
        dotfiles_dir,
        packages,
        opts,
        &adapters::FileStoreImpl,
        &reporter,
    )
}

/// Link packages with injectable filesystem and reporter (for testing).
pub fn link_with_store(
    dotfiles_dir: &Path,
    packages: &[String],
    opts: &LinkOptions,
    fs: &dyn FileStore,
    reporter: &dyn Reporter,
) -> Result<State, NotfilesError> {
    let config = notcore::Config::load(dotfiles_dir)?;
    config.validate()?;
    let mut state = linker::State::load(dotfiles_dir, fs)?;
    let pkgs = package::resolve_packages_filtered_with_store(dotfiles_dir, packages, &config, fs)?;
    for pkg in &pkgs {
        linker::link_package(dotfiles_dir, &config, &mut state, pkg, opts, fs, reporter)?;
    }
    state.save(dotfiles_dir, fs)?;
    Ok(state)
}

/// Remove managed symlinks/copies for all (or specified) packages.
pub fn unlink(
    dotfiles_dir: &Path,
    packages: &[String],
    opts: &LinkOptions,
) -> Result<(), NotfilesError> {
    let reporter = adapters::TerminalReporter;
    unlink_with_store(
        dotfiles_dir,
        packages,
        opts,
        &adapters::FileStoreImpl,
        &reporter,
    )
}

/// Unlink packages with injectable filesystem and reporter.
pub fn unlink_with_store(
    dotfiles_dir: &Path,
    packages: &[String],
    opts: &LinkOptions,
    fs: &dyn FileStore,
    reporter: &dyn Reporter,
) -> Result<(), NotfilesError> {
    let mut state = linker::State::load(dotfiles_dir, fs)?;
    let pkgs = if packages.is_empty() {
        state
            .entries
            .iter()
            .map(|e| e.package.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        packages.to_vec()
    };
    for pkg in &pkgs {
        linker::unlink_package(dotfiles_dir, &mut state, pkg, opts, fs, reporter)?;
    }
    state.save(dotfiles_dir, fs)?;
    Ok(())
}
