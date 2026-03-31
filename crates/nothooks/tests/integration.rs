use std::fs;
use tempfile::TempDir;
use nothooks::{HookResult, HookRunner};
use notcore::{HookPhase, HookSpec};

fn make_hook_script(dir: &TempDir, name: &str, content: &str) -> HookSpec {
    let path = dir.path().join(format!("{name}.nu"));
    fs::write(&path, content).unwrap();
    HookSpec {
        name: name.to_string(),
        script: path.to_str().unwrap().to_string(),
        phase: HookPhase::Dot,
    }
}

#[test]
fn test_hook_success() {
    let dir = TempDir::new().unwrap();
    let spec = make_hook_script(&dir, "ok-hook", "print hello");
    let runner = HookRunner::new(dir.path().to_path_buf());
    let result = runner.run_hook(&spec);
    assert!(matches!(result, HookResult::Ok));
}

#[test]
fn test_hook_failure() {
    let dir = TempDir::new().unwrap();
    let spec = make_hook_script(&dir, "fail-hook", "exit 1\n");
    let runner = HookRunner::new(dir.path().to_path_buf());
    let result = runner.run_hook(&spec);
    assert!(matches!(result, HookResult::Failed(_)));
}

#[test]
fn test_setup_hook_skipped_on_rerun() {
    let dir = TempDir::new().unwrap();
    let mut spec = make_hook_script(&dir, "setup-hook", "print ran");
    spec.phase = notcore::HookPhase::Setup;

    let runner = HookRunner::new(dir.path().to_path_buf());
    let r1 = runner.run_hook(&spec);
    assert!(matches!(r1, HookResult::Ok));

    // Second run — should be skipped
    let r2 = runner.run_hook(&spec);
    assert!(matches!(r2, HookResult::Skipped));
}

#[test]
fn test_setup_hook_force_reruns() {
    let dir = TempDir::new().unwrap();
    let mut spec = make_hook_script(&dir, "force-hook", "print ran");
    spec.phase = notcore::HookPhase::Setup;

    let runner = HookRunner::new(dir.path().to_path_buf());
    runner.run_hook(&spec);

    let runner2 = HookRunner::with_force(dir.path().to_path_buf());
    let r2 = runner2.run_hook(&spec);
    assert!(matches!(r2, HookResult::Ok));
}
