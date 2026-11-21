use nothooks::adapters::{FsStateStore, InMemoryStateStore, ProcessRunner};
use nothooks::ports::{ScriptInvocation, ScriptRunner, StateStore};
use nothooks::state::HookState;
use std::path::Path;
use tempfile::TempDir;

fn assert_state_store_contract(store: &dyn StateStore) {
    let fresh = store.load().expect("load on a never-saved store");
    assert_eq!(fresh, HookState::default(), "never-saved store is empty");

    let mut state = HookState::default();
    state.mark_done("alpha");
    state.mark_done("beta");
    store.save(&state).expect("save");
    assert_eq!(store.load().expect("load after save"), state);

    state.mark_done("gamma");
    store.save(&state).expect("overwrite");
    let reloaded = store.load().expect("load after overwrite");
    assert!(reloaded.is_done("alpha") && reloaded.is_done("gamma"));
    assert_eq!(reloaded, state);
}

#[test]
fn fs_state_store_satisfies_contract() {
    let dir = TempDir::new().expect("tempdir");
    assert_state_store_contract(&FsStateStore::new(dir.path()));
}

#[test]
fn in_memory_state_store_satisfies_contract() {
    assert_state_store_contract(&InMemoryStateStore::default());
}

#[test]
fn fs_state_store_leaves_no_temp_file_behind() {
    let dir = TempDir::new().expect("tempdir");
    let store = FsStateStore::new(dir.path());
    store.save(&HookState::default()).expect("save");

    let names: Vec<String> = std::fs::read_dir(dir.path())
        .expect("read dir")
        .map(|e| {
            e.expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, vec![".nothooks-state.toml".to_string()]);
}

fn run_sh(script: &Path, env: &[(String, String)]) -> std::io::Result<nothooks::ports::ScriptExit> {
    ProcessRunner.run(&ScriptInvocation {
        interpreter: "sh",
        script,
        env,
    })
}

fn script_in(dir: &TempDir, body: &str) -> std::path::PathBuf {
    let path = dir.path().join("hook.sh");
    std::fs::write(&path, body).expect("write script");
    path
}

#[test]
fn process_runner_reports_success_and_exit_codes() {
    let dir = TempDir::new().expect("tempdir");

    let ok = run_sh(&script_in(&dir, "exit 0\n"), &[]).expect("runs");
    assert!(ok.success());

    let failed = run_sh(&script_in(&dir, "exit 3\n"), &[]).expect("runs");
    assert!(!failed.success());
    assert_eq!(failed.code, Some(3));
}

#[test]
fn process_runner_passes_env_with_later_entries_winning() {
    let dir = TempDir::new().expect("tempdir");
    let script = script_in(&dir, "test \"$NF_PROBE\" = second\n");
    let env = vec![
        ("NF_PROBE".to_string(), "first".to_string()),
        ("NF_PROBE".to_string(), "second".to_string()),
    ];

    assert!(run_sh(&script, &env).expect("runs").success());
}

#[test]
fn process_runner_errors_when_the_interpreter_is_missing() {
    let dir = TempDir::new().expect("tempdir");
    let script = script_in(&dir, "exit 0\n");

    let result = ProcessRunner.run(&ScriptInvocation {
        interpreter: "nothooks-no-such-interpreter",
        script: &script,
        env: &[],
    });

    assert!(
        result.is_err(),
        "unstartable process is Err, not a failed exit"
    );
}
