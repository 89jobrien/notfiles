//! Core types and configuration for the notfiles dotfiles manager.
//!
//! This crate provides shared types used across the notfiles workspace:
//! configuration parsing, error types, path utilities, and the reporter
//! trait for output abstraction.

pub mod config;
pub mod error;
pub mod paths;
pub mod reporter;
pub mod types;

pub use config::{Config, Defaults, Method, PackageConfig, suggest_package};
pub use error::NotfilesError;
pub use paths::{dotfiles_dir, expand_tilde};
pub use reporter::{LinkEvent, Reporter, SilentReporter};
pub use types::{HookPhase, HookSpec, PackageSpec, Report, Step, StepStatus};
