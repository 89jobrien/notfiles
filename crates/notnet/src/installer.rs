use crate::error::NotnetError;

/// Returns true if the `tailscale` binary is on PATH.
pub fn is_installed() -> bool {
    std::process::Command::new("tailscale")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Install Tailscale using the platform package manager, falling back to the official
/// install script.
///
/// Supported package managers (tried in order): `apt-get`, `brew`, `pacman`.
/// Fallback: `curl -fsSL https://tailscale.com/install.sh | sh`.
pub fn install() -> Result<(), NotnetError> {
    if try_apt().is_ok() {
        return Ok(());
    }
    if try_brew().is_ok() {
        return Ok(());
    }
    if try_pacman().is_ok() {
        return Ok(());
    }
    install_via_script()
}

fn try_apt() -> Result<(), NotnetError> {
    // apt-get install tailscale requires the tailscale repo to be configured already.
    // We only attempt this if apt-get is present — the user is expected to have added
    // the Tailscale apt repo as part of their base image or preseed.
    if !cmd_exists("apt-get") {
        return Err(not_available("apt-get"));
    }
    run_cmd("apt-get", &["install", "-y", "tailscale"])
}

fn try_brew() -> Result<(), NotnetError> {
    if !cmd_exists("brew") {
        return Err(not_available("brew"));
    }
    run_cmd("brew", &["install", "tailscale"])
}

fn try_pacman() -> Result<(), NotnetError> {
    if !cmd_exists("pacman") {
        return Err(not_available("pacman"));
    }
    run_cmd("pacman", &["-S", "--noconfirm", "tailscale"])
}

fn install_via_script() -> Result<(), NotnetError> {
    // Pipe the official install script through sh.
    // `curl | sh` is the documented method on tailscale.com/download.
    let output = std::process::Command::new("sh")
        .args(["-c", "curl -fsSL https://tailscale.com/install.sh | sh"])
        .output()
        .map_err(|e| NotnetError::InstallFailed(e.to_string()))?;
    if !output.status.success() {
        return Err(NotnetError::InstallFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    Ok(())
}

fn cmd_exists(cmd: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", &format!("command -v {cmd}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_cmd(program: &str, args: &[&str]) -> Result<(), NotnetError> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|e| NotnetError::InstallFailed(e.to_string()))?;
    if !output.status.success() {
        return Err(NotnetError::InstallFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    Ok(())
}

fn not_available(name: &str) -> NotnetError {
    NotnetError::InstallFailed(format!("{name} not found"))
}
