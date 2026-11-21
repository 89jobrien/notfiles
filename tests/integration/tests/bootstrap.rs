use notcore::{HookPhase, HookSpec, StepStatus};
use nothooks::{HookRunner, run_phase};
use notstrap::{BootstrapOptions, run};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

mod support;
use support::write_throwaway_age_key;

struct TestEnv {
    dotfiles: TempDir, // kept alive to prevent early drop of temp directory
    home: TempDir,
    config: PathBuf,
    key_file: PathBuf,
}

fn make_test_env() -> TestEnv {
    let dotfiles = TempDir::new().expect("failed to create temp dotfiles dir");
    let home = TempDir::new().expect("failed to create temp home dir");
    let d = dotfiles.path();

    // age key file (throwaway identity derived at run time)
    let key_file = d.join("age.key");
    write_throwaway_age_key(&key_file);

    // notfiles.toml — one package "shell" targeting home tempdir
    fs::write(
        d.join("notfiles.toml"),
        format!(
            "[defaults]\nmethod = \"symlink\"\ntarget = \"{}\"\n",
            home.path().display()
        ),
    )
    .expect("write should succeed");

    // a file to link inside a package dir
    let pkg = d.join("shell");
    fs::create_dir_all(&pkg).expect("create_dir_all should succeed");
    fs::write(pkg.join(".zshrc"), "# test zshrc\n").expect("write should succeed");

    // scripts dir for hooks
    fs::create_dir_all(d.join("scripts")).expect("create_dir_all should succeed");

    // notstrap.toml — dotfiles_dir points to existing tempdir so clone is skipped
    let config = d.join("notstrap.toml");
    fs::write(
        &config,
        format!(
            "[bootstrap]\ndotfiles_repo = \"https://example.com/fake.git\"\ndotfiles_dir = \"{}\"\n",
            d.display()
        ),
    )
    .expect("write should succeed");

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
        tailscale: Some(None), // skip Tailscale in tests
        check_prereqs: None,
        reconcile_tools: None,
        git: Box::new(notstrap::repo::SystemGit),
        prompter: None,
        assume_yes: false,
        secrets_config: None,
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
    fs::write(&script, "print hello\n").expect("write should succeed");

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
    .expect("write should succeed");

    let report = run(make_opts(&env, false)).expect("run should succeed");
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
    const HOOK_NAME: &str = "install-tools";
    let env = make_test_env();
    let d = env.dotfiles.path();

    let script = d.join("scripts/setup.nu");
    fs::write(&script, "print setup_ran\n").expect("write should succeed");

    let config_content = format!(
        "[bootstrap]\n\
         dotfiles_repo = \"https://example.com/fake.git\"\n\
         dotfiles_dir = \"{dotfiles}\"\n\n\
         [[hooks]]\n\
         name = \"{HOOK_NAME}\"\n\
         script = \"{script}\"\n\
         phase = \"setup\"\n",
        dotfiles = d.display(),
        script = script.display(),
    );
    fs::write(&env.config, &config_content).expect("write should succeed");

    // ── First run ──────────────────────────────────────────────────────────
    let r1 = run(make_opts(&env, false)).expect("run should succeed");
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
    let state_content = fs::read_to_string(&state_file).expect("read_to_string should succeed");
    assert!(
        state_content.contains(HOOK_NAME),
        "state file should record {HOOK_NAME} as done, got: {state_content}"
    );

    // ── Second run ─────────────────────────────────────────────────────────
    let r2 = run(make_opts(&env, false)).expect("run should succeed");
    assert!(
        !r2.has_failures(),
        "second run should not fail: {:?}",
        r2.steps
    );

    // Verify skip behaviour directly via nothooks — HookRunner should skip
    // the setup hook because state file already marks it done.
    let hook_spec = HookSpec {
        name: HOOK_NAME.to_string(),
        script: script.to_str().expect("to_str should succeed").to_string(),
        phase: HookPhase::Setup,
        interpreter: None,
    };
    let runner = HookRunner::new(d.to_path_buf());
    let phase_report =
        run_phase(&[hook_spec], &HookPhase::Setup, &runner).expect("run_phase should succeed");
    let step = phase_report
        .steps
        .iter()
        .find(|s| s.name == HOOK_NAME)
        .expect("install-tools step should be in phase report");
    assert_eq!(
        step.status,
        StepStatus::Skipped,
        "{HOOK_NAME} should be skipped on second run"
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
        tailscale: Some(None), // skip Tailscale in tests
        check_prereqs: None,
        reconcile_tools: None,
        git: Box::new(notstrap::repo::SystemGit),
        prompter: None,
        assume_yes: false,
        secrets_config: None,
    };

    let report = run(opts).expect("run() should return Ok even when a step fails");

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

/// A link conflict must stop bootstrap before any hook phase runs.
#[test]
fn test_bootstrap_stops_after_link_failure() {
    let env = make_test_env();
    let d = env.dotfiles.path();
    let home = env.home.path();

    fs::write(home.join(".zshrc"), "local override\n").expect("write should succeed");

    let script = d.join("scripts/greet.nu");
    fs::write(&script, "print hello\n").expect("write should succeed");
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
    .expect("write should succeed");

    let report = run(make_opts(&env, false)).expect("run should succeed");

    let link_step = report
        .steps
        .iter()
        .find(|step| step.name == "link dotfiles")
        .expect("link dotfiles step should be present");
    assert!(
        matches!(link_step.status, StepStatus::Failed(_)),
        "link failure should be surfaced, got {:?}",
        link_step.status
    );
    assert!(
        report.steps.iter().all(|step| step.name != "dot hooks"),
        "bootstrap should stop before running hooks after link failure: {:?}",
        report.steps
    );
}
