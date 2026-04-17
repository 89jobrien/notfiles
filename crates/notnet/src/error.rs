use thiserror::Error;

#[derive(Debug, Error)]
pub enum NotnetError {
    #[error("Tailscale is not installed; set install = true in [tailscale] to auto-install")]
    NotInstalled,

    #[error("Tailscale install failed: {0}")]
    InstallFailed(String),

    #[error("command `{cmd}` failed: {detail}")]
    CommandFailed { cmd: String, detail: String },

    #[error("peer `{0}` is unreachable over Tailscale")]
    PeerUnreachable(String),

    #[error(
        "auth key resolution failed: no TS_AUTHKEY env var, no YubiKey on slot 9d, and user declined prompt"
    )]
    NoAuthKey,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
