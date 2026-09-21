use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Diagnostic, Error)]
pub enum NotshellError {
    #[error("shell IR file `{path}` could not be parsed: {detail}")]
    #[diagnostic(code(notshell::parse))]
    ParseFailed { path: String, detail: String },

    #[error("no emitter registered for shell `{0}`")]
    #[diagnostic(code(notshell::unknown_shell))]
    UnknownShell(String),

    #[error("I/O error: {0}")]
    #[diagnostic(code(notshell::io))]
    Io(#[from] std::io::Error),
}
