pub mod cli;
pub mod ignore;
pub mod linker;
pub mod package;
pub mod status;

use anyhow::Result;
use std::path::Path;

pub use linker::{LinkOptions, State};
pub use package::resolve_packages;

pub fn link(
    dotfiles_dir: &Path,
    packages: &[String],
    opts: &LinkOptions,
) -> Result<State> {
    let config = notcore::Config::load(dotfiles_dir)?;
    let mut state = State::load(dotfiles_dir)?;
    let pkgs = resolve_packages(dotfiles_dir, packages)?;
    for pkg in &pkgs {
        linker::link_package(dotfiles_dir, &config, &mut state, pkg, opts)?;
    }
    state.save(dotfiles_dir)?;
    Ok(state)
}

pub fn unlink(dotfiles_dir: &Path, packages: &[String], opts: &LinkOptions) -> Result<()> {
    let mut state = State::load(dotfiles_dir)?;
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
        linker::unlink_package(dotfiles_dir, &mut state, pkg, opts)?;
    }
    state.save(dotfiles_dir)?;
    Ok(())
}
