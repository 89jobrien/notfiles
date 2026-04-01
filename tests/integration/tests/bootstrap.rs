use notcore::{HookPhase, HookSpec, StepStatus};
use nothooks::{run_phase, HookRunner};
use notstrap::{run, BootstrapOptions};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

struct TestEnv {
    dotfiles: TempDir,
    home: TempDir,
    config: PathBuf,
    key_file: PathBuf,
}

fn make_test_env() -> TestEnv {
    let dotfiles = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let d = dotfiles.path();

    // age key file (content doesn't matter — FileSource reads it verbatim)
    let key_file = d.join("age.key");
    fs::write(&key_file, "AGE-SECRET-KEY-1TESTKEY\n").unwrap();

    // notfiles.toml — one package "shell" targeting home tempdir
    fs::write(
        d.join("notfiles.toml"),
        format!(
            "[defaults]\nmethod = \"symlink\"\ntarget = \"{}\"\n",
            home.path().display()
        ),
    )
    .unwrap();

    // a file to link inside a package dir
    let pkg = d.join("shell");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(pkg.join(".zshrc"), "# test zshrc\n").unwrap();

    // scripts dir for hooks
    fs::create_dir_all(d.join("scripts")).unwrap();

    // notstrap.toml — dotfiles_dir points to existing tempdir so clone is skipped
    let config = d.join("notstrap.toml");
    fs::write(
        &config,
        format!(
            "[bootstrap]\ndotfiles_repo = \"https://example.com/fake.git\"\ndotfiles_dir = \"{}\"\n",
            d.display()
        ),
    )
    .unwrap();

    TestEnv {
        dotfiles,
        home,
        config,
        key_file,
    }
}

/// Build a BootstrapOptions pointing at env, with optional force flag.
fn make_opts(env: &TestEnv, force: bool) -> BootstrapOptions {
    BootstrapOptions {
        config: env.config.clone(),
        force,
        key_file: Some(env.key_file.clone()),
        dotfiles: Some(env.dotfiles.path().to_path_buf()),
        check_prereqs: None,
        env_injector: None,
    }
}

// ── Test 1 ────────────────────────────────────────────────────────────────────

/// Full bootstrap with a dot hook succeeds and symlinks .zshrc into home.
#[test]
fn test_full_bootstrap_dot_hooks_only() {
    let env = make_test_env();
    let d = env.dotfiles.path();

    // Add a dot hook script
    let script = d.join("scripts/greet.nu");
    fs::write(&script, "print hello\n").unwrap();

    // Rewrite notstrap.toml to include the dot hook
    fs::write(
        &env.config,
        format!(
            "[bootstrap]\n\
             dotfiles_repo = \"https://example.com/fake.git\"\n\
             dotfiles_dir = \"{dotfiles}\"\n\n\
             [[hooks]]\n\
             name = \"greet\"\n\
             script = \"{script}\"\n\
             phase = \"dot\"\n",
            dotfiles = d.display(),
            script = script.display(),
        ),
    )
    .unwrap();

    let report = run(make_opts(&env, false)).unwrap();
    assert!(
        !report.has_failures(),
        "unexpected failures: {:?}",
        report
            .steps
            .iter()
            .filter(|s| matches!(s.status, StepStatus::Failed(_)))
            .collect::<Vec<_>>()
    );

    // .zshrc should be symlinked into home
    let link_target = env.home.path().join(".zshrc");
    assert!(link_target.exists(), ".zshrc should exist in home");
    assert!(
        link_target.is_symlink(),
        ".zshrc should be a symlink (not a copy)"
    );
}

// ── Test 2 ────────────────────────────────────────────────────────────────────

/// Setup hooks run on first bootstrap then are skipped on subsequent runs.
///
/// notstrap::run() only exposes per-phase summaries in the top-level Report,
/// so we verify skip behaviour by inspecting .nothooks-state.toml after the
/// first run and by calling nothooks::run_phase directly after the second run.
#[test]
fn test_setup_hooks_skipped_on_rerun() {
    let env = make_test_env();
    let d = env.dotfiles.path();

    let script = d.join("scripts/setup.nu");
    fs::write(&script, "print setup_ran\n").unwrap();

    let config_content = format!(
        "[bootstrap]\n\
         dotfiles_repo = \"https://example.com/fake.git\"\n\
         dotfiles_dir = \"{dotfiles}\"\n\n\
         [[hooks]]\n\
         name = \"install-tools\"\n\
         script = \"{script}\"\n\
         phase = \"setup\"\n",
        dotfiles = d.display(),
        script = script.display(),
    );
    fs::write(&env.config, &config_content).unwrap();

    // ── First run ──────────────────────────────────────────────────────────
    let r1 = run(make_opts(&env, false)).unwrap();
    assert!(
        !r1.has_failures(),
        "first run should not fail: {:?}",
        r1.steps
    );

    // State file must exist and record the hook as done
    let state_file = d.join(".nothooks-state.toml");
    assert!(
        state_file.exists(),
        ".nothooks-state.toml must be written after first run"
    );
    let state_content = fs::read_to_string(&state_file).unwrap();
    assert!(
        state_content.contains("install-tools"),
        "state file should record install-tools as done, got: {state_content}"
    );

    // ── Second run ─────────────────────────────────────────────────────────
    let r2 = run(make_opts(&env, false)).unwrap();
    assert!(
        !r2.has_failures(),
        "second run should not fail: {:?}",
        r2.steps
    );

    // Verify skip behaviour directly via nothooks — HookRunner should skip
    // the setup hook because state file already marks it done.
    let hook_spec = HookSpec {
        name: "install-tools".to_string(),
        script: script.to_str().unwrap().to_string(),
        phase: HookPhase::Setup,
    };
    let runner = HookRunner::new(d.to_path_buf());
    let phase_report = run_phase(&[hook_spec], &HookPhase::Setup, &runner);
    let step = phase_report
        .steps
        .iter()
        .find(|s| s.name == "install-tools")
        .expect("install-tools step should be in phase report");
    assert_eq!(
        step.status,
        StepStatus::Skipped,
        "install-tools should be skipped on second run"
    );
}

// ── Test 3 ────────────────────────────────────────────────────────────────────

/// A nonexistent key file causes the age-key step to fail and stops early
/// (no link-dotfiles step is recorded).
#[test]
fn test_bootstrap_fails_fast_on_bad_key() {
    let env = make_test_env();
    let d = env.dotfiles.path();

    let opts = BootstrapOptions {
        config: env.config.clone(),
        force: false,
        key_file: Some(PathBuf::from("/nonexistent/no-such-key.age")),
        dotfiles: Some(d.to_path_buf()),
        check_prereqs: None,
        env_injector: None,
    };

    let report = run(opts).unwrap();

    // age key step must fail
    let key_step = report
        .steps
        .iter()
        .find(|s| s.name == "age key")
        .expect("age key step should be present");
    assert!(
        matches!(key_step.status, StepStatus::Failed(_)),
        "bad key path should fail age key step, got: {:?}",
        key_step.status
    );

    // no link step should appear (bootstrap stops early after key failure)
    let has_link = report
        .steps
        .iter()
        .any(|s| s.name.starts_with("link dotfiles"));
    assert!(
        !has_link,
        "should not reach link dotfiles step after key failure"
    );
}
