use notnet::NotnetError;
use notnet::adapters::{FakeCommandRunner, TailscaleCli};
use notnet::ports::Tailscale;

#[test]
fn is_connected_asks_for_status_without_peers() {
    let runner = FakeCommandRunner::default();
    assert!(TailscaleCli::new(&runner).is_connected());
    assert_eq!(runner.calls(), ["tailscale status --peers=false"]);
}

#[test]
fn is_connected_is_false_when_status_fails_or_cannot_start() {
    let failing = FakeCommandRunner::default().failing("tailscale status --peers=false", "down");
    assert!(!TailscaleCli::new(&failing).is_connected());

    let missing = FakeCommandRunner::default().unspawnable("tailscale");
    assert!(!TailscaleCli::new(&missing).is_connected());
}

#[test]
fn is_installed_runs_tailscale_version() {
    let runner = FakeCommandRunner::default();
    assert!(TailscaleCli::new(&runner).is_installed());
    assert_eq!(runner.calls(), ["tailscale version"]);

    let missing = FakeCommandRunner::default().unspawnable("tailscale");
    assert!(!TailscaleCli::new(&missing).is_installed());
}

#[test]
fn join_passes_the_auth_key_via_a_file_flag() {
    let runner = FakeCommandRunner::default();
    TailscaleCli::new(&runner).join("tskey-abc").expect("join");
    let calls = runner.calls();
    assert_eq!(calls.len(), 1);
    assert!(
        calls[0].starts_with("tailscale up --auth-key=file:"),
        "unexpected call: {:?}",
        calls[0]
    );
    // The key itself must not appear in the process arguments.
    assert!(!calls[0].contains("tskey-abc"));
}

#[test]
fn join_failure_carries_stderr() {
    let runner = FakeCommandRunner::default().failing("tailscale up", "bad key");
    let err = TailscaleCli::new(&runner).join("k").expect_err("fails");

    assert!(
        matches!(err, NotnetError::CommandFailed { ref cmd, ref detail }
            if cmd == "tailscale up" && detail == "bad key")
    );
}

#[test]
fn join_spawn_failure_is_a_command_failure() {
    let runner = FakeCommandRunner::default().unspawnable("tailscale");
    let err = TailscaleCli::new(&runner).join("k").expect_err("fails");
    assert!(matches!(err, NotnetError::CommandFailed { .. }));
}

#[test]
fn verify_peer_pings_once() {
    let runner = FakeCommandRunner::default();
    TailscaleCli::new(&runner)
        .verify_peer("minibox")
        .expect("reachable");
    assert_eq!(runner.calls(), ["tailscale ping --c 1 minibox"]);
}

#[test]
fn verify_peer_failure_is_peer_unreachable() {
    let runner = FakeCommandRunner::default().failing("tailscale ping --c 1 minibox", "timeout");
    let err = TailscaleCli::new(&runner)
        .verify_peer("minibox")
        .expect_err("unreachable");
    assert!(matches!(err, NotnetError::PeerUnreachable(h) if h == "minibox"));
}
