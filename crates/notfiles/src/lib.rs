pub mod adapters;
pub mod cli;
pub mod ignore;
pub mod linker;
pub mod package;
pub mod ports;
pub mod status;

use anyhow::Result;
use std::path::Path;

pub use adapters::FileStoreImpl;
pub use linker::{LinkOptions, State};
pub use package::{resolve_packages, resolve_packages_with_store};
pub use ports::FileStore;

pub fn link(dotfiles_dir: &Path, packages: &[String], opts: &LinkOptions) -> Result<State> {
    link_with_store(dotfiles_dir, packages, opts, &adapters::FileStoreImpl)
}

pub fn link_with_store(
    dotfiles_dir: &Path,
    packages: &[String],
    opts: &LinkOptions,
    fs: &dyn FileStore,
) -> Result<State> {
    let config = notcore::Config::load(dotfiles_dir)?;
    let mut state = linker::State::load(dotfiles_dir, fs)?;
    let pkgs = resolve_packages_with_store(dotfiles_dir, packages, fs)?;
    for pkg in &pkgs {
        linker::link_package(dotfiles_dir, &config, &mut state, pkg, opts, fs)?;
    }
    state.save(dotfiles_dir, fs)?;
    Ok(state)
}

pub fn unlink(dotfiles_dir: &Path, packages: &[String], opts: &LinkOptions) -> Result<()> {
    unlink_with_store(dotfiles_dir, packages, opts, &adapters::FileStoreImpl)
}

pub fn unlink_with_store(
    dotfiles_dir: &Path,
    packages: &[String],
    opts: &LinkOptions,
    fs: &dyn FileStore,
) -> Result<()> {
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
        linker::unlink_package(dotfiles_dir, &mut state, pkg, opts, fs)?;
    }
    state.save(dotfiles_dir, fs)?;
    Ok(())
}
