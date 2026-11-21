use crate::error::NotnetError;
use std::io;
use std::time::Duration;

/// Default bound for one process run, applied by implementations that can enforce it.
pub const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// What a finished child process reported. `stderr` is lossily decoded text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub success: bool,
    pub stderr: String,
}

/// Process boundary: the only place that spawns commands.
///
/// Contract: `Err` means the program could not be started; a program that ran and failed
/// is `Ok` with `success == false`. [`CommandRunner::run_timeout`] additionally returns
/// `Err` with `ErrorKind::TimedOut` when a run exceeds its bound.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> io::Result<CommandOutput>;
    /// [`CommandRunner::run`] bounded by `timeout`: implementations without a bound
    /// return the result of `run`.
    fn run_timeout(
        &self,
        program: &str,
        args: &[&str],
        _timeout: Duration,
    ) -> io::Result<CommandOutput> {
        self.run(program, args)
    }
    /// Whether an executable named `program` resolves on `PATH`.
    fn exists(&self, program: &str) -> bool;
}

/// The local Tailscale node, as the bootstrap flow needs it.
pub trait Tailscale {
    fn is_connected(&self) -> bool;
    fn is_installed(&self) -> bool;
    /// Joins the node with `auth_key`. Implementations pass the key to the CLI
    /// without putting it in the process argv.
    fn join(&self, auth_key: &str) -> Result<(), NotnetError>;
    /// `Err(PeerUnreachable)` when the peer does not answer.
    ///
    /// TODO (suggested): consider an alternative reachability check that does not
    /// depend on ICMP — some networks block ping while the tailnet stays up;
    /// `tailscale status --json` lists peers' online state.
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
