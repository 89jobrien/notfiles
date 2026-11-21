use notnet::adapters::SystemCommandRunner;
use notnet::ports::CommandRunner;

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
fn exists_reflects_path_lookup() {
    assert!(SystemCommandRunner.exists("sh"));
    assert!(!SystemCommandRunner.exists("notnet-no-such-program"));
}
