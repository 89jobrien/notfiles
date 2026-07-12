use serde::Deserialize;

/// Configuration for Tailscale network setup.
#[derive(Debug, Clone, Deserialize)]
pub struct TailscaleOptions {
    /// Tailscale hostname of the peer to verify after joining (e.g. "minibox").
    pub peer_hostname: String,
    /// Gitea clone URL reachable over the tailnet (e.g. "http://minibox:3000/joe/dotfiles.git").
    pub gitea_url: String,
    /// If true, install Tailscale automatically when the binary is missing.
    #[serde(default)]
    pub install: bool,
}
