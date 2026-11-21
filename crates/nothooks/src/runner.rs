use crate::HookResult;
use crate::adapters::{FsStateStore, ProcessRunner};
use crate::ports::{ScriptInvocation, ScriptRunner, StateStore};
use crate::state::HookState;
use anyhow::{Context, Result};
use notcore::{HookPhase, HookSpec, Report, StepStatus};
use std::path::{Path, PathBuf};

fn resolve_interpreter(spec: &HookSpec) -> Result<String, String> {
    if let Some(interp) = &spec.interpreter {
        return Ok(interp.clone());
    }
    let ext = std::path::Path::new(&spec.script)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "nu" => Ok("nu".into()),
        "rs" => Ok("rust-script".into()),
        "sh" => Ok("sh".into()),
        "bash" => Ok("bash".into()),
        "zsh" => Ok("zsh".into()),
        "rb" => Ok("ruby".into()),
        _ => Err(format!(
            "cannot infer interpreter: no interpreter set and unknown extension {:?}",
            ext
        )),
    }
}

pub struct HookRunner {
    state_dir: PathBuf,
    force: bool,
    extra_env: Vec<(String, String)>,
    scripts: Box<dyn ScriptRunner>,
    store: Box<dyn StateStore>,
}

impl HookRunner {
    pub fn new(state_dir: PathBuf) -> Self {
        let store = Box::new(FsStateStore::new(&state_dir));
        Self::with_ports(state_dir, Box::new(ProcessRunner), store)
    }

    pub fn with_force(state_dir: PathBuf) -> Self {
        Self::new(state_dir).forced()
    }

    /// Build a runner over explicit adapters; `new` wires the real ones.
    pub fn with_ports(
        state_dir: PathBuf,
        scripts: Box<dyn ScriptRunner>,
        store: Box<dyn StateStore>,
    ) -> Self {
        Self {
            state_dir,
            force: false,
            extra_env: Vec::new(),
            scripts,
            store,
        }
    }

    /// Re-run setup hooks even when they already completed.
    pub fn forced(mut self) -> Self {
        self.force = true;
        self
    }

    /// Environment variables added to every hook process, e.g. `NOTSTRAP_YES=1`.
    pub fn with_env(mut self, vars: Vec<(String, String)>) -> Self {
        self.extra_env = vars;
        self
    }

    /// Run all hooks in `hooks` that match `phase`.
    ///
    /// State is loaded once before iterating and saved once at the end (only if
    /// at least one Setup hook completed successfully), avoiding N file reads
    /// and writes per phase.
    pub fn run_phase(&self, hooks: &[HookSpec], phase: &HookPhase) -> Result<Report> {
        let mut state = self.load_state()?;
        let mut state_dirty = false;
        let mut report = Report::default();

        for spec in hooks.iter().filter(|h| &h.phase == phase) {
            let result = self.run_hook_with_state(spec, &mut state, &mut state_dirty);
            let status = match &result {
                HookResult::Ok => StepStatus::Ok,
                HookResult::Skipped => StepStatus::Skipped,
                HookResult::Failed(msg) => StepStatus::Failed(msg.clone()),
            };
            report.add(&spec.name, status);
        }

        if state_dirty {
            self.save_state(&state)?;
        }

        Ok(report)
    }

    /// Execute a single hook, using the caller-owned `state` and `dirty` flag
    /// instead of loading/saving per invocation.
    fn run_hook_with_state(
        &self,
        spec: &HookSpec,
        state: &mut HookState,
        state_dirty: &mut bool,
    ) -> HookResult {
        if spec.phase == HookPhase::Setup && !self.force && state.is_done(&spec.name) {
            return HookResult::Skipped;
        }

        let interp = match resolve_interpreter(spec) {
            Ok(i) => i,
            Err(msg) => return HookResult::Failed(msg),
        };
        let script = self.resolve_script_path(&spec.script);
        // Hooks locate the dotfiles checkout through NOTFILES_DIR, not their own path.
        // Extra env goes last so it can override.
        let mut env = vec![(
            "NOTFILES_DIR".to_string(),
            self.state_dir.to_string_lossy().into_owned(),
        )];
        env.extend(self.extra_env.iter().cloned());

        let result = self.scripts.run(&ScriptInvocation {
            interpreter: &interp,
            script: &script,
            env: &env,
        });

        match result {
            Ok(exit) if exit.success() => {
                if spec.phase == HookPhase::Setup {
                    state.mark_done(&spec.name);
                    *state_dirty = true;
                }
                HookResult::Ok
            }
            Ok(exit) => HookResult::Failed(format!("exit code {}", exit.code.unwrap_or(-1))),
            Err(e) => HookResult::Failed(e.to_string()),
        }
    }

    fn load_state(&self) -> Result<HookState> {
        self.store
            .load()
            .with_context(|| format!("loading hook state for {}", self.state_dir.display()))
    }

    fn save_state(&self, state: &HookState) -> Result<()> {
        self.store
            .save(state)
            .with_context(|| format!("saving hook state for {}", self.state_dir.display()))
    }

    /// Run a single hook with its own load/save cycle.
    ///
    /// Prefer [`HookRunner::run_phase`] when running multiple hooks to avoid
    /// repeated state I/O.
    pub fn run_hook(&self, spec: &HookSpec) -> Result<HookResult> {
        let mut state = self.load_state()?;
        let mut state_dirty = false;
        let result = self.run_hook_with_state(spec, &mut state, &mut state_dirty);
        if state_dirty {
            self.save_state(&state)?;
        }
        Ok(result)
    }

    fn resolve_script_path(&self, script: &str) -> PathBuf {
        let script = Path::new(script);
        if script.is_absolute() {
            script.to_path_buf()
        } else {
            self.state_dir.join(script)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notcore::HookPhase;

    use crate::adapters::InMemoryStateStore;
    use crate::ports::{ScriptExit, ScriptInvocation, ScriptRunner};
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::io;
    use std::rc::Rc;

    #[derive(Debug, Clone, PartialEq)]
    struct Recorded {
        interpreter: String,
        script: PathBuf,
        env: Vec<(String, String)>,
    }

    /// Records every invocation and replays scripted outcomes (default: exit 0).
    #[derive(Clone, Default)]
    struct FakeRunner {
        calls: Rc<RefCell<Vec<Recorded>>>,
        outcomes: Rc<RefCell<VecDeque<io::Result<ScriptExit>>>>,
    }

    impl FakeRunner {
        fn exits(self, code: i32) -> Self {
            self.outcomes
                .borrow_mut()
                .push_back(Ok(ScriptExit { code: Some(code) }));
            self
        }

        fn fails_to_spawn(self, msg: &str) -> Self {
            self.outcomes.borrow_mut().push_back(Err(io::Error::new(
                io::ErrorKind::NotFound,
                msg.to_string(),
            )));
            self
        }

        fn calls(&self) -> Vec<Recorded> {
            self.calls.borrow().clone()
        }
    }

    impl ScriptRunner for FakeRunner {
        fn run(&self, invocation: &ScriptInvocation<'_>) -> io::Result<ScriptExit> {
            self.calls.borrow_mut().push(Recorded {
                interpreter: invocation.interpreter.to_string(),
                script: invocation.script.to_path_buf(),
                env: invocation.env.to_vec(),
            });
            self.outcomes
                .borrow_mut()
                .pop_front()
                .unwrap_or(Ok(ScriptExit { code: Some(0) }))
        }
    }

    fn runner_with(fake: &FakeRunner, store: &InMemoryStateStore, force: bool) -> HookRunner {
        let runner = HookRunner::with_ports(
            PathBuf::from("/dots"),
            Box::new(fake.clone()),
            Box::new(store.clone()),
        );
        if force { runner.forced() } else { runner }
    }

    fn named(name: &str, script: &str, phase: HookPhase) -> HookSpec {
        HookSpec {
            name: name.into(),
            script: script.into(),
            phase,
            interpreter: None,
        }
    }

    fn spec(script: &str, interpreter: Option<&str>) -> HookSpec {
        HookSpec {
            name: "t".into(),
            script: script.into(),
            phase: HookPhase::Setup,
            interpreter: interpreter.map(str::to_string),
        }
    }

    #[test]
    fn setup_hook_runs_once_then_is_skipped() {
        let (fake, store) = (FakeRunner::default(), InMemoryStateStore::default());
        let runner = runner_with(&fake, &store, false);
        let hooks = [named("a", "a.nu", HookPhase::Setup)];

        runner
            .run_phase(&hooks, &HookPhase::Setup)
            .expect("first run");
        let report = runner
            .run_phase(&hooks, &HookPhase::Setup)
            .expect("second run");

        assert_eq!(fake.calls().len(), 1);
        assert!(store.is_done("a"));
        assert!(!report.has_failures());
    }

    #[test]
    fn force_reruns_completed_setup_hook() {
        let (fake, store) = (FakeRunner::default(), InMemoryStateStore::default());
        let hooks = [named("a", "a.nu", HookPhase::Setup)];

        runner_with(&fake, &store, false)
            .run_phase(&hooks, &HookPhase::Setup)
            .expect("first run");
        runner_with(&fake, &store, true)
            .run_phase(&hooks, &HookPhase::Setup)
            .expect("forced run");

        assert_eq!(fake.calls().len(), 2);
    }

    #[test]
    fn dot_hook_always_runs_and_never_touches_state() {
        let (fake, store) = (FakeRunner::default(), InMemoryStateStore::default());
        let runner = runner_with(&fake, &store, false);
        let hooks = [named("d", "d.nu", HookPhase::Dot)];

        runner
            .run_phase(&hooks, &HookPhase::Dot)
            .expect("first run");
        runner
            .run_phase(&hooks, &HookPhase::Dot)
            .expect("second run");

        assert_eq!(fake.calls().len(), 2);
        assert_eq!(store.saves(), 0);
        assert!(!store.is_done("d"));
    }

    #[test]
    fn failed_setup_hook_is_not_marked_done() {
        let (fake, store) = (
            FakeRunner::default().exits(3),
            InMemoryStateStore::default(),
        );
        let runner = runner_with(&fake, &store, false);

        let result = runner
            .run_hook(&named("a", "a.nu", HookPhase::Setup))
            .expect("state io");

        assert_eq!(result, HookResult::Failed("exit code 3".into()));
        assert!(!store.is_done("a"));
        assert_eq!(store.saves(), 0);
    }

    #[test]
    fn spawn_error_is_reported_as_failure() {
        let fake = FakeRunner::default().fails_to_spawn("no such interpreter");
        let store = InMemoryStateStore::default();
        let runner = runner_with(&fake, &store, false);

        let result = runner
            .run_hook(&named("a", "a.nu", HookPhase::Setup))
            .expect("state io");

        assert_eq!(result, HookResult::Failed("no such interpreter".into()));
    }

    #[test]
    fn unresolvable_interpreter_fails_without_running_anything() {
        let (fake, store) = (FakeRunner::default(), InMemoryStateStore::default());
        let runner = runner_with(&fake, &store, false);

        let result = runner
            .run_hook(&named("a", "a.xyz", HookPhase::Setup))
            .expect("state io");

        assert!(matches!(result, HookResult::Failed(_)));
        assert!(fake.calls().is_empty());
    }

    /// Regression: state is loaded once and saved once per phase, not per hook.
    #[test]
    fn run_phase_batches_state_io() {
        let (fake, store) = (FakeRunner::default(), InMemoryStateStore::default());
        let runner = runner_with(&fake, &store, false);
        let hooks = [
            named("a", "a.nu", HookPhase::Setup),
            named("b", "b.nu", HookPhase::Setup),
            named("c", "c.nu", HookPhase::Setup),
        ];

        runner.run_phase(&hooks, &HookPhase::Setup).expect("run");

        assert_eq!(store.loads(), 1);
        assert_eq!(store.saves(), 1);
    }

    #[test]
    fn run_phase_does_not_save_when_nothing_completed() {
        let (fake, store) = (FakeRunner::default(), InMemoryStateStore::default());
        let runner = runner_with(&fake, &store, false);
        let hooks = [named("a", "a.nu", HookPhase::Setup)];

        runner
            .run_phase(&hooks, &HookPhase::Setup)
            .expect("first run");
        let saves_after_first = store.saves();
        runner
            .run_phase(&hooks, &HookPhase::Setup)
            .expect("second run");

        assert_eq!(saves_after_first, 1);
        assert_eq!(store.saves(), 1);
    }

    #[test]
    fn run_phase_only_runs_hooks_of_the_requested_phase() {
        let (fake, store) = (FakeRunner::default(), InMemoryStateStore::default());
        let runner = runner_with(&fake, &store, false);
        let hooks = [
            named("a", "a.nu", HookPhase::Setup),
            named("d", "d.nu", HookPhase::Dot),
        ];

        runner.run_phase(&hooks, &HookPhase::Dot).expect("run");

        let calls = fake.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].script, PathBuf::from("/dots/d.nu"));
    }

    /// Regression: hooks find the checkout through NOTFILES_DIR.
    #[test]
    fn hooks_receive_notfiles_dir_and_extra_env() {
        let (fake, store) = (FakeRunner::default(), InMemoryStateStore::default());
        let runner =
            runner_with(&fake, &store, false).with_env(vec![("NOTSTRAP_YES".into(), "1".into())]);

        runner
            .run_hook(&named("a", "a.nu", HookPhase::Dot))
            .expect("state io");

        let env = &fake.calls()[0].env;
        assert!(env.contains(&("NOTFILES_DIR".into(), "/dots".into())));
        assert!(env.contains(&("NOTSTRAP_YES".into(), "1".into())));
    }

    #[test]
    fn relative_script_resolves_from_state_dir_and_absolute_is_untouched() {
        let (fake, store) = (FakeRunner::default(), InMemoryStateStore::default());
        let runner = runner_with(&fake, &store, false);

        runner
            .run_hook(&named("r", "hooks/r.nu", HookPhase::Dot))
            .expect("state io");
        runner
            .run_hook(&named("x", "/abs/x.nu", HookPhase::Dot))
            .expect("state io");

        let calls = fake.calls();
        assert_eq!(calls[0].script, PathBuf::from("/dots/hooks/r.nu"));
        assert_eq!(calls[1].script, PathBuf::from("/abs/x.nu"));
    }

    #[test]
    fn rs_hooks_run_through_rust_script() {
        let (fake, store) = (FakeRunner::default(), InMemoryStateStore::default());
        let runner = runner_with(&fake, &store, false);

        runner
            .run_hook(&named("a", "a.rs", HookPhase::Dot))
            .expect("state io");

        assert_eq!(fake.calls()[0].interpreter, "rust-script");
    }

    #[test]
    fn infers_interpreter_from_extension() {
        assert_eq!(
            resolve_interpreter(&spec("a.nu", None)).expect("nu is known"),
            "nu"
        );
        assert_eq!(
            resolve_interpreter(&spec("a.rs", None)).expect("rs is known"),
            "rust-script"
        );
    }

    #[test]
    fn explicit_interpreter_wins() {
        assert_eq!(
            resolve_interpreter(&spec("a.rs", Some("custom"))).expect("explicit interpreter"),
            "custom"
        );
    }

    #[test]
    fn unknown_extension_is_an_error() {
        assert!(resolve_interpreter(&spec("a.xyz", None)).is_err());
    }

    /// Python is not a supported hook language; it needs an explicit `interpreter`.
    #[test]
    fn python_scripts_are_not_inferred() {
        assert!(resolve_interpreter(&spec("a.py", None)).is_err());
        assert_eq!(
            resolve_interpreter(&spec("a.py", Some("custom"))).expect("explicit interpreter"),
            "custom"
        );
    }
}
