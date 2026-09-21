//! Error types shared by the notfiles workspace.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum NotfilesError {
    #[error("config file error: {0}")]
    Config(String),

    #[error("package not found: {name}")]
    PackageNotFound { name: String },

    #[error("conflict at {path}: {reason}")]
    Conflict { path: PathBuf, reason: String },

    #[error("path error: {0}")]
    Path(String),

    #[error("state file error: {0}")]
    State(String),

    #[error("glob error: {0}")]
    Glob(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("shell config generation error: {0}")]
    Shell(String),

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}
