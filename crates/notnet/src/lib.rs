pub mod auth;
pub mod error;
pub mod installer;
pub mod ports;

pub use error::NotnetError;
pub use ports::TailscaleOptions;

use anyhow::Result;

/// Ensure this machine is connected to the tailnet and the target peer is reachable.
///
/// Returns immediately (skipped) if `tailscale status` reports an active connection.
/// Installs Tailscale if `opts.install` is true and the binary is missing.
pub fn ensure_connected(opts: &TailscaleOptions) -> Result<bool, NotnetError> {
    // Already connected — fast path.
    if status::is_connected() {
        return Ok(false); // false = skipped (already on tailnet)
    }

    // Install if missing and opts.install allows it.
    if !installer::is_installed() {
        if !opts.install {
            return Err(NotnetError::NotInstalled);
        }
        installer::install()?;
    }

    // Resolve auth key.
    let auth_key = auth::resolve_auth_key()?;

    // Join the tailnet.
    join(&auth_key)?;

    // Verify target peer is reachable.
    verify_peer(&opts.peer_hostname)?;

    Ok(true) // true = connected now
}

fn join(auth_key: &str) -> Result<(), NotnetError> {
    let output = std::process::Command::new("tailscale")
        .args(["up", &format!("--authkey={auth_key}")])
        .output()
        .map_err(|e| NotnetError::CommandFailed {
            cmd: "tailscale up".to_string(),
            detail: e.to_string(),
        })?;
    if !output.status.success() {
        return Err(NotnetError::CommandFailed {
            cmd: "tailscale up".to_string(),
            detail: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

fn verify_peer(hostname: &str) -> Result<(), NotnetError> {
    let output = std::process::Command::new("tailscale")
        .args(["ping", "--c", "1", hostname])
        .output()
        .map_err(|e| NotnetError::CommandFailed {
            cmd: format!("tailscale ping {hostname}"),
            detail: e.to_string(),
        })?;
    if !output.status.success() {
        return Err(NotnetError::PeerUnreachable(hostname.to_string()));
    }
    Ok(())
}

mod status {
    pub fn is_connected() -> bool {
        std::process::Command::new("tailscale")
            .args(["status", "--peers=false"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}
