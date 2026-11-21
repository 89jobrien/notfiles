//! Tailscale network presence management for notstrap.
//!
//! [`ensure_connected`] drives the `tailscale` CLI (through the
//! [`CommandRunner`](ports::CommandRunner) port, whose real adapter bounds
//! every invocation with a timeout) to bring the machine onto the tailnet and
//! verify the target peer is reachable. Auth keys come from the
//! [`AuthKeySource`](ports::AuthKeySource) chain: `TS_AUTHKEY`, then the YubiKey
//! PIV slot 9d (when compiled with the `yubikey` feature), then an interactive
//! prompt.

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
use std::time::Duration;

/// How many extra times peer verification is retried after `tailscale up`.
///
/// `tailscale up --auth-key` returns while the node may not yet be reporting as
/// connected; the retries tolerate that lag before declaring the peer
/// unreachable.
pub const VERIFY_PEER_RETRIES: usize = 3;

/// Delay between peer-verification retries.
pub const VERIFY_PEER_RETRY_DELAY: Duration = Duration::from_millis(250);

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
/// `Ok(false)` means the node was already on the tailnet; `Ok(true)` means it
/// joined now. Peer verification retries briefly, because `tailscale up` can
/// return while the node still reports as disconnected.
///
/// TODO (suggested): add a `Tailscale::status()` snapshot (self hostname,
/// backend state, per-peer online states) so callers can report what the node
/// is connected to; parse it from `tailscale status --json` once notnet gains a
/// JSON dependency.
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

    // Joining is eventually consistent: `tailscale up` returns while the node
    // may not yet be connected, so the first `ping` can fail. Retry briefly
    // before declaring the peer unreachable.
    let mut verify = tailscale.verify_peer(&opts.peer_hostname);
    let mut retries = 0;
    while verify.is_err() && retries < VERIFY_PEER_RETRIES {
        retries += 1;
        std::thread::sleep(VERIFY_PEER_RETRY_DELAY);
        verify = tailscale.verify_peer(&opts.peer_hostname);
    }
    verify?;
    Ok(true)
}
