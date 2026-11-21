use notnet::ports::{AuthKeySource, Tailscale, TailscaleInstaller};
use notnet::{NotnetError, TailscaleOptions, VERIFY_PEER_RETRIES, ensure_connected_with};
use std::cell::RefCell;
use std::rc::Rc;

type Log = Rc<RefCell<Vec<String>>>;

struct FakeTailscale {
    log: Log,
    connected: bool,
    installed: bool,
    join_error: bool,
    peer_reachable: bool,
}

impl FakeTailscale {
    fn new(log: &Log) -> Self {
        Self {
            log: log.clone(),
            connected: false,
            installed: true,
            join_error: false,
            peer_reachable: true,
        }
    }
}

impl Tailscale for FakeTailscale {
    fn is_connected(&self) -> bool {
        self.log.borrow_mut().push("is_connected".into());
        self.connected
    }

    fn is_installed(&self) -> bool {
        self.log.borrow_mut().push("is_installed".into());
        self.installed
    }

    fn join(&self, auth_key: &str) -> Result<(), NotnetError> {
        self.log.borrow_mut().push(format!("join:{auth_key}"));
        if self.join_error {
            return Err(NotnetError::CommandFailed {
                cmd: "tailscale up".into(),
                detail: "denied".into(),
            });
        }
        Ok(())
    }

    fn verify_peer(&self, hostname: &str) -> Result<(), NotnetError> {
        self.log.borrow_mut().push(format!("verify:{hostname}"));
        if self.peer_reachable {
            Ok(())
        } else {
            Err(NotnetError::PeerUnreachable(hostname.into()))
        }
    }
}

struct FakeInstaller(Log);

impl TailscaleInstaller for FakeInstaller {
    fn install(&self) -> Result<(), NotnetError> {
        self.0.borrow_mut().push("install".into());
        Ok(())
    }
}

struct FakeAuth {
    log: Log,
    key: Option<&'static str>,
}

impl AuthKeySource for FakeAuth {
    fn auth_key(&self) -> Result<String, NotnetError> {
        self.log.borrow_mut().push("auth_key".into());
        self.key.map(str::to_string).ok_or(NotnetError::NoAuthKey)
    }
}

fn opts(install: bool) -> TailscaleOptions {
    TailscaleOptions {
        peer_hostname: "minibox".into(),
        gitea_url: "http://minibox:3000/joe/dotfiles.git".into(),
        install,
    }
}

fn run(
    ts: FakeTailscale,
    log: &Log,
    key: Option<&'static str>,
    install: bool,
) -> Result<bool, NotnetError> {
    ensure_connected_with(
        &opts(install),
        &ts,
        &FakeInstaller(log.clone()),
        &FakeAuth {
            log: log.clone(),
            key,
        },
    )
}

fn events(log: &Log) -> Vec<String> {
    log.borrow().clone()
}

#[test]
fn already_connected_skips_everything_else() {
    let log = Log::default();
    let mut ts = FakeTailscale::new(&log);
    ts.connected = true;

    assert!(!run(ts, &log, Some("k"), true).expect("connected"));
    assert_eq!(events(&log), ["is_connected"]);
}

#[test]
fn installed_but_disconnected_joins_and_verifies_without_installing() {
    let log = Log::default();
    let result = run(FakeTailscale::new(&log), &log, Some("k"), false);

    assert!(result.expect("joined"));
    assert_eq!(
        events(&log),
        [
            "is_connected",
            "is_installed",
            "auth_key",
            "join:k",
            "verify:minibox"
        ]
    );
}

#[test]
fn missing_binary_without_install_permission_is_an_error() {
    let log = Log::default();
    let mut ts = FakeTailscale::new(&log);
    ts.installed = false;

    let err = run(ts, &log, Some("k"), false).expect_err("must refuse");

    assert!(matches!(err, NotnetError::NotInstalled));
    assert!(
        !events(&log)
            .iter()
            .any(|e| e == "install" || e.starts_with("join"))
    );
}

#[test]
fn missing_binary_with_install_permission_installs_first() {
    let log = Log::default();
    let mut ts = FakeTailscale::new(&log);
    ts.installed = false;

    assert!(run(ts, &log, Some("k"), true).expect("joined"));
    assert_eq!(
        events(&log),
        [
            "is_connected",
            "is_installed",
            "install",
            "auth_key",
            "join:k",
            "verify:minibox"
        ]
    );
}

#[test]
fn auth_key_failure_stops_before_joining() {
    let log = Log::default();
    let err = run(FakeTailscale::new(&log), &log, None, false).expect_err("no key");

    assert!(matches!(err, NotnetError::NoAuthKey));
    assert!(!events(&log).iter().any(|e| e.starts_with("join")));
}

#[test]
fn join_failure_stops_before_verifying_the_peer() {
    let log = Log::default();
    let mut ts = FakeTailscale::new(&log);
    ts.join_error = true;

    let err = run(ts, &log, Some("k"), false).expect_err("join fails");

    assert!(matches!(err, NotnetError::CommandFailed { .. }));
    assert!(!events(&log).iter().any(|e| e.starts_with("verify")));
}

#[test]
fn unreachable_peer_is_reported_with_its_hostname() {
    let log = Log::default();
    let mut ts = FakeTailscale::new(&log);
    ts.peer_reachable = false;

    let err = run(ts, &log, Some("k"), false).expect_err("unreachable");

    assert!(matches!(err, NotnetError::PeerUnreachable(h) if h == "minibox"));
}

#[test]
fn verify_peer_is_retried_before_reporting_unreachable() {
    let log = Log::default();
    let mut ts = FakeTailscale::new(&log);
    ts.peer_reachable = false;

    let err = run(ts, &log, Some("k"), false).expect_err("unreachable");

    assert!(matches!(err, NotnetError::PeerUnreachable(ref h) if h == "minibox"));
    let verifies = events(&log)
        .iter()
        .filter(|e| e.starts_with("verify:"))
        .count();
    assert_eq!(verifies, 1 + VERIFY_PEER_RETRIES);
}
