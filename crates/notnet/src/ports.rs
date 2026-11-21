use crate::error::NotnetError;
use std::io;

/// What a finished child process reported. `stderr` is lossily decoded text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub success: bool,
    pub stderr: String,
}

/// Process boundary: the only place that spawns commands.
///
/// Contract: `Err` means the program could not be started; a program that ran and failed
/// is `Ok` with `success == false`.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> io::Result<CommandOutput>;
    /// Whether an executable named `program` resolves on `PATH`.
    fn exists(&self, program: &str) -> bool;
}

/// The local Tailscale node, as the bootstrap flow needs it.
pub trait Tailscale {
    fn is_connected(&self) -> bool;
    fn is_installed(&self) -> bool;
    fn join(&self, auth_key: &str) -> Result<(), NotnetError>;
    /// `Err(PeerUnreachable)` when the peer does not answer.
    fn verify_peer(&self, hostname: &str) -> Result<(), NotnetError>;
}

/// Puts the Tailscale binary on the machine.
pub trait TailscaleInstaller {
    fn install(&self) -> Result<(), NotnetError>;
}

/// Produces a Tailscale auth key, or `Err(NoAuthKey)` when every source is exhausted.
pub trait AuthKeySource {
    fn auth_key(&self) -> Result<String, NotnetError>;
}
