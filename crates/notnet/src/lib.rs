pub mod adapters;
pub mod auth;
pub mod error;
pub mod options;
pub mod ports;

pub use error::NotnetError;
pub use options::TailscaleOptions;

use adapters::{PackageManagerInstaller, SystemCommandRunner, TailscaleCli};
use auth::ChainedAuthKey;
use ports::{AuthKeySource, Tailscale, TailscaleInstaller};

/// Ensure this machine is connected to the tailnet and the target peer is reachable.
///
/// Returns immediately (skipped) if `tailscale status` reports an active connection.
/// Installs Tailscale if `opts.install` is true and the binary is missing.
pub fn ensure_connected(opts: &TailscaleOptions) -> Result<bool, NotnetError> {
    let runner = SystemCommandRunner;
    ensure_connected_with(
        opts,
        &TailscaleCli::new(&runner),
        &PackageManagerInstaller::new(&runner),
        &ChainedAuthKey,
    )
}

/// [`ensure_connected`] over explicit adapters.
///
/// `Ok(false)` means the node was already on the tailnet; `Ok(true)` means it joined now.
pub fn ensure_connected_with(
    opts: &TailscaleOptions,
    tailscale: &dyn Tailscale,
    installer: &dyn TailscaleInstaller,
    auth: &dyn AuthKeySource,
) -> Result<bool, NotnetError> {
    if tailscale.is_connected() {
        return Ok(false);
    }

    if !tailscale.is_installed() {
        if !opts.install {
            return Err(NotnetError::NotInstalled);
        }
        installer.install()?;
    }

    let auth_key = auth.auth_key()?;
    tailscale.join(&auth_key)?;
    tailscale.verify_peer(&opts.peer_hostname)?;

    Ok(true)
}
