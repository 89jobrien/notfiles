//! Reconcile declared tools with what is actually installed.
//!
//! `nottools` reads the tool manifests a dotfiles repo already keeps (a mise config and
//! Brewfiles), scans shell config files for aliases, and reports:
//!
//! - alias targets that do not resolve on `PATH`, with the install command when the tool
//!   is declared in mise or a Brewfile,
//! - `cargo install` crates that no manifest declares, split by registry, git and path source.
//!
//! Hexagonal layout: [`ports`] define the filesystem, probe and installer boundaries,
//! [`adapters`] provide system and in-memory implementations, and the remaining modules are
//! pure domain logic.

pub mod adapters;
pub mod aliases;
pub mod config;
pub mod declared;
pub mod doctor;
pub mod fix;
pub mod ports;

pub use config::DoctorConfig;
pub use doctor::{Backend, DoctorReport, MissingTool, run};
pub use fix::{FixAction, adopt, apply_fixes};
pub use ports::{Fs, InstalledCrate, Installer, Source, ToolProbe};
