use std::process::Command;

fn notfiles_bin() -> std::path::PathBuf {
    // Built by `cargo test`
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // remove test binary name
    path.pop(); // remove `deps`
    path.push("notfiles");
    path
}

/// `notfiles bootstrap` must delegate into notstrap::run and surface its
/// error path through the CLI, proving the notfiles-cli wiring works without
/// needing real secrets or a real dotfiles clone.
#[test]
fn bootstrap_subcommand_delegates_to_notstrap_and_fails_on_missing_config() {
    let output = Command::new(notfiles_bin())
        .args(["bootstrap", "--config", "/nonexistent/notstrap.toml"])
        .output()
        .expect("failed to run notfiles binary");

    assert!(
        !output.status.success(),
        "bootstrap with a missing config should exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot read"),
        "expected notstrap's config-read error to surface, got: {stderr}"
    );
}
