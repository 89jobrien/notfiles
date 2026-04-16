use notcore::{HookPhase, HookSpec};
use nothooks::{HookResult, HookRunner};
use std::fs;
use tempfile::TempDir;

fn make_hook_script(dir: &TempDir, name: &str, content: &str) -> HookSpec {
    let path = dir.path().join(format!("{name}.sh"));
    fs::write(&path, content).unwrap();
    HookSpec {
        name: name.to_string(),
        script: path.to_str().unwrap().to_string(),
        phase: HookPhase::Dot,
        interpreter: None,
    }
}

#[test]
fn test_hook_success() {
    let dir = TempDir::new().unwrap();
    let spec = make_hook_script(&dir, "ok-hook", "echo hello");
    let runner = HookRunner::new(dir.path().to_path_buf());
    let result = runner.run_hook(&spec).unwrap();
    assert!(matches!(result, HookResult::Ok));
}

#[test]
fn test_hook_failure() {
    let dir = TempDir::new().unwrap();
    let spec = make_hook_script(&dir, "fail-hook", "exit 1");
    let runner = HookRunner::new(dir.path().to_path_buf());
    let result = runner.run_hook(&spec).unwrap();
    assert!(matches!(result, HookResult::Failed(_)));
}

#[test]
fn test_setup_hook_skipped_on_rerun() {
    let dir = TempDir::new().unwrap();
    let mut spec = make_hook_script(&dir, "setup-hook", "echo ran");
    spec.phase = notcore::HookPhase::Setup;

    let runner = HookRunner::new(dir.path().to_path_buf());
    let r1 = runner.run_hook(&spec).unwrap();
    assert!(matches!(r1, HookResult::Ok));

    // Second run — should be skipped
    let r2 = runner.run_hook(&spec).unwrap();
    assert!(matches!(r2, HookResult::Skipped));
}

#[test]
fn test_setup_hook_force_reruns() {
    let dir = TempDir::new().unwrap();
    let mut spec = make_hook_script(&dir, "force-hook", "echo ran");
    spec.phase = notcore::HookPhase::Setup;

    let runner = HookRunner::new(dir.path().to_path_buf());
    runner.run_hook(&spec).unwrap();

    let runner2 = HookRunner::with_force(dir.path().to_path_buf());
    let r2 = runner2.run_hook(&spec).unwrap();
    assert!(matches!(r2, HookResult::Ok));
}

#[test]
fn test_relative_script_path_resolves_from_state_dir() {
    let dir = TempDir::new().unwrap();
    let scripts_dir = dir.path().join("scripts");
    fs::create_dir_all(&scripts_dir).unwrap();
    let script = scripts_dir.join("relative.sh");
    fs::write(&script, "echo relative-ok").unwrap();

    let spec = HookSpec {
        name: "relative".to_string(),
        script: "scripts/relative.sh".to_string(),
        phase: HookPhase::Dot,
        interpreter: Some("sh".to_string()),
    };

    let runner = HookRunner::new(dir.path().to_path_buf());
    let result = runner.run_hook(&spec).unwrap();
    assert!(matches!(result, HookResult::Ok));
}
