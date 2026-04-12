use crate::HookResult;
use crate::state::HookState;
use notcore::{HookPhase, HookSpec, Report, StepStatus};
use std::path::PathBuf;
use std::process::Command;

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
        "sh" => Ok("sh".into()),
        "bash" => Ok("bash".into()),
        "zsh" => Ok("zsh".into()),
        "py" => Ok("python3".into()),
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
}

impl HookRunner {
    pub fn new(state_dir: PathBuf) -> Self {
        Self {
            state_dir,
            force: false,
        }
    }

    pub fn with_force(state_dir: PathBuf) -> Self {
        Self {
            state_dir,
            force: true,
        }
    }

    /// Run all hooks in `hooks` that match `phase`.
    ///
    /// State is loaded once before iterating and saved once at the end (only if
    /// at least one Setup hook completed successfully), avoiding N file reads
    /// and writes per phase.
    pub fn run_phase(&self, hooks: &[HookSpec], phase: &HookPhase) -> Report {
        let mut state = HookState::load(&self.state_dir).unwrap_or_default();
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
            let _ = state.save(&self.state_dir);
        }

        report
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
        let result = Command::new(&interp).arg(&spec.script).status();

        match result {
            Ok(status) if status.success() => {
                if spec.phase == HookPhase::Setup {
                    state.mark_done(&spec.name);
                    *state_dirty = true;
                }
                HookResult::Ok
            }
            Ok(status) => HookResult::Failed(format!("exit code {}", status.code().unwrap_or(-1))),
            Err(e) => HookResult::Failed(e.to_string()),
        }
    }

    /// Run a single hook with its own load/save cycle.
    ///
    /// Prefer [`HookRunner::run_phase`] when running multiple hooks to avoid
    /// repeated state I/O.
    pub fn run_hook(&self, spec: &HookSpec) -> HookResult {
        let mut state = HookState::load(&self.state_dir).unwrap_or_default();
        let mut state_dirty = false;
        let result = self.run_hook_with_state(spec, &mut state, &mut state_dirty);
        if state_dirty {
            let _ = state.save(&self.state_dir);
        }
        result
    }
}
