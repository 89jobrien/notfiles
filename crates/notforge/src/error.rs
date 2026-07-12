use miette::Diagnostic;
use thiserror::Error;

/// Error type for forge lifecycle, API, git, and secret-resolution failures.
#[derive(Debug, Diagnostic, Error)]
pub enum NotforgeError {
    /// Configuration could not be loaded or parsed.
    #[error("configuration error: {0}")]
    #[diagnostic(code(notforge::config))]
    Config(String),

    /// A required secret could not be resolved.
    #[error("secret resolution error: {0}")]
    #[diagnostic(code(notforge::secret))]
    Secret(String),

    /// Forge API communication failed.
    #[error("HTTP error: {0}")]
    #[diagnostic(code(notforge::http))]
    Http(String),

    /// A lifecycle command failed.
    #[error("command error: {0}")]
    #[diagnostic(code(notforge::command))]
    Command(String),

    /// A git operation failed.
    #[error("git error: {0}")]
    #[diagnostic(code(notforge::git))]
    Git(String),

    /// Repository lookup or provisioning failed.
    #[error("repository error: {0}")]
    #[diagnostic(code(notforge::repository))]
    Repository(String),
}
