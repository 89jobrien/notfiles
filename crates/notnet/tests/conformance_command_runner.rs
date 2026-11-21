use notnet::adapters::SystemCommandRunner;
use notnet::ports::CommandRunner;
use std::io;
use std::time::Duration;

#[test]
fn successful_command_reports_success() {
    let out = SystemCommandRunner
        .run("sh", &["-c", "exit 0"])
        .expect("runs");
    assert!(out.success);
}

#[test]
fn failing_command_reports_failure_and_stderr() {
    let out = SystemCommandRunner
        .run("sh", &["-c", "echo boom >&2; exit 1"])
        .expect("runs");
    assert!(!out.success);
    assert_eq!(out.stderr.trim(), "boom");
}

#[test]
fn unstartable_command_is_err_not_a_failed_output() {
    assert!(
        SystemCommandRunner
            .run("notnet-no-such-program", &[])
            .is_err()
    );
}

#[test]
fn run_timeout_bounds_a_slow_command() {
    let err = SystemCommandRunner
        .run_timeout("sh", &["-c", "sleep 5"], Duration::from_millis(100))
        .expect_err("should time out");
    assert_eq!(err.kind(), io::ErrorKind::TimedOut);
}

#[test]
fn run_timeout_passes_through_normal_results() {
    let out = SystemCommandRunner
        .run_timeout(
            "sh",
            &["-c", "echo boom >&2; exit 1"],
            Duration::from_secs(5),
        )
        .expect("runs");
    assert!(!out.success);
    assert_eq!(out.stderr.trim(), "boom");
}

#[test]
fn run_timeout_is_err_for_an_unstartable_program() {
    assert!(
        SystemCommandRunner
            .run_timeout("notnet-no-such-program", &[], Duration::from_secs(5))
            .is_err()
    );
}

#[test]
fn exists_reflects_path_lookup() {
    assert!(SystemCommandRunner.exists("sh"));
    assert!(!SystemCommandRunner.exists("notnet-no-such-program"));
}
