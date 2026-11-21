use crate::error::NotnetError;
use crate::ports::{
    CommandOutput, CommandRunner, DEFAULT_COMMAND_TIMEOUT, Tailscale, TailscaleInstaller,
};
use std::cell::RefCell;
use std::io;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;

/// Bound applied to every `tailscale` CLI invocation.
const TAILSCALE_CMD_TIMEOUT: Duration = Duration::from_secs(30);

/// Poll interval while waiting for a child in [`SystemCommandRunner::run_timeout`].
const RUN_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Bound applied to package-manager installs: they can take minutes.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(600);

/// Spawns real child processes.
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> io::Result<CommandOutput> {
        self.run_timeout(program, args, DEFAULT_COMMAND_TIMEOUT)
    }

    /// Bounded run: the child is killed and `Err(ErrorKind::TimedOut)` returned
    /// when it does not exit within `timeout`.
    ///
    /// The child's stdout is nulled: `CommandOutput` carries only stderr, and a
    /// full stdout pipe could otherwise deadlock a long-running child we meant
    /// to time out.
    fn run_timeout(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> io::Result<CommandOutput> {
        let mut child = Command::new(program)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        let deadline = Instant::now() + timeout;
        loop {
            match child.try_wait()? {
                Some(status) => {
                    let mut stderr = Vec::new();
                    if let Some(mut err) = child.stderr.take() {
                        err.read_to_end(&mut stderr)?;
                    }
                    return Ok(CommandOutput {
                        success: status.success(),
                        stderr: String::from_utf8_lossy(&stderr).into_owned(),
                    });
                }
                None => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            format!("`{program}` did not finish within {timeout:?}"),
                        ));
                    }
                    std::thread::sleep(RUN_POLL_INTERVAL);
                }
            }
        }
    }

    fn exists(&self, program: &str) -> bool {
        // `program` is a positional parameter, never interpolated into the script.
        self.run("sh", &["-c", "command -v \"$1\"", "sh", program])
            .map(|o| o.success)
            .unwrap_or(false)
    }
}

/// Drives the `tailscale` CLI through a `CommandRunner`.
pub struct TailscaleCli<'a> {
    runner: &'a dyn CommandRunner,
}

impl<'a> TailscaleCli<'a> {
    pub fn new(runner: &'a dyn CommandRunner) -> Self {
        Self { runner }
    }

    fn run(&self, args: &[&str]) -> io::Result<CommandOutput> {
        self.runner
            .run_timeout("tailscale", args, TAILSCALE_CMD_TIMEOUT)
    }

    fn succeeds(&self, args: &[&str]) -> bool {
        self.run(args).map(|o| o.success).unwrap_or(false)
    }
}

impl Tailscale for TailscaleCli<'_> {
    fn is_connected(&self) -> bool {
        self.succeeds(&["status", "--peers=false"])
    }

    fn is_installed(&self) -> bool {
        self.succeeds(&["version"])
    }

    fn join(&self, auth_key: &str) -> Result<(), NotnetError> {
        let failed = |detail: String| NotnetError::CommandFailed {
            cmd: "tailscale up".to_string(),
            detail,
        };
        // Pass the key as `--auth-key file:<path>` instead of `--authkey <key>`:
        // argv is visible to other processes on the machine.
        let key_file = NamedTempFile::new()?;
        std::fs::write(key_file.path(), auth_key)?;
        let flag = format!("--auth-key=file:{}", key_file.path().display());
        let output = self
            .run(&["up", &flag])
            .map_err(|e| failed(e.to_string()))?;
        if output.success {
            Ok(())
        } else {
            Err(failed(output.stderr))
        }
    }

    fn verify_peer(&self, hostname: &str) -> Result<(), NotnetError> {
        let output =
            self.run(&["ping", "--c", "1", hostname])
                .map_err(|e| NotnetError::CommandFailed {
                    cmd: format!("tailscale ping {hostname}"),
                    detail: e.to_string(),
                })?;
        if output.success {
            Ok(())
        } else {
            Err(NotnetError::PeerUnreachable(hostname.to_string()))
        }
    }
}

/// Installs Tailscale with the platform package manager, falling back to the official script.
///
/// Managers are tried in order (`apt-get`, `brew`, `pacman`); the first that is present and
/// succeeds wins. Fallback: `curl -fsSL https://tailscale.com/install.sh | sh`.
pub struct PackageManagerInstaller<'a> {
    runner: &'a dyn CommandRunner,
}

impl<'a> PackageManagerInstaller<'a> {
    pub fn new(runner: &'a dyn CommandRunner) -> Self {
        Self { runner }
    }

    fn run_ok(&self, program: &str, args: &[&str]) -> Result<(), NotnetError> {
        let output = self
            .runner
            .run_timeout(program, args, INSTALL_TIMEOUT)
            .map_err(|e| NotnetError::InstallFailed(e.to_string()))?;
        if output.success {
            Ok(())
        } else {
            Err(NotnetError::InstallFailed(output.stderr))
        }
    }

    fn try_manager(&self, program: &str, args: &[&str]) -> Result<(), NotnetError> {
        if !self.runner.exists(program) {
            return Err(NotnetError::InstallFailed(format!("{program} not found")));
        }
        self.run_ok(program, args)
    }
}

impl TailscaleInstaller for PackageManagerInstaller<'_> {
    fn install(&self) -> Result<(), NotnetError> {
        // apt-get needs the tailscale repo configured already; we only try it when present.
        let managers: [(&str, &[&str]); 3] = [
            ("apt-get", &["install", "-y", "tailscale"]),
            ("brew", &["install", "tailscale"]),
            ("pacman", &["-S", "--noconfirm", "tailscale"]),
        ];
        for (program, args) in managers {
            if self.try_manager(program, args).is_ok() {
                return Ok(());
            }
        }
        // `curl | sh` is the documented method on tailscale.com/download.
        self.run_ok(
            "sh",
            &["-c", "curl -fsSL https://tailscale.com/install.sh | sh"],
        )
    }
}

/// Test double for `CommandRunner`: records each command line and replays scripted outcomes.
///
/// A command line is the program followed by its space-joined args. Everything succeeds
/// unless scripted otherwise; `exists` is true only for programs named in `present`.
#[derive(Default)]
pub struct FakeCommandRunner {
    present: Vec<String>,
    failing: Vec<(String, String)>,
    unspawnable: Vec<String>,
    calls: RefCell<Vec<String>>,
}

impl FakeCommandRunner {
    pub fn present(mut self, programs: &[&str]) -> Self {
        self.present.extend(programs.iter().map(|p| p.to_string()));
        self
    }

    /// `command_line` (matched as a prefix of the recorded line) exits non-zero with `stderr`.
    pub fn failing(mut self, command_line: &str, stderr: &str) -> Self {
        self.failing
            .push((command_line.to_string(), stderr.to_string()));
        self
    }

    /// Spawning `program` fails with `NotFound`.
    pub fn unspawnable(mut self, program: &str) -> Self {
        self.unspawnable.push(program.to_string());
        self
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl CommandRunner for FakeCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> io::Result<CommandOutput> {
        let line = std::iter::once(program)
            .chain(args.iter().copied())
            .collect::<Vec<_>>()
            .join(" ");
        self.calls.borrow_mut().push(line.clone());
        if self.unspawnable.iter().any(|p| p == program) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{program}: not found"),
            ));
        }
        let failure = self
            .failing
            .iter()
            .find(|(l, _)| line.starts_with(l.as_str()));
        Ok(CommandOutput {
            success: failure.is_none(),
            stderr: failure.map(|(_, e)| e.clone()).unwrap_or_default(),
        })
    }

    fn exists(&self, program: &str) -> bool {
        self.present.iter().any(|p| p == program)
    }
}
