pub mod config;
pub mod error;
pub mod paths;
pub mod types;

pub use config::{Config, Defaults, Method, PackageConfig};
pub use error::NotfilesError;
pub use paths::{dotfiles_dir, expand_tilde};
pub use types::{HookPhase, HookSpec, PackageSpec, Report, Step, StepStatus};
