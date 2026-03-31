use std::path::PathBuf;
use std::process::Command;
use notcore::{HookPhase, HookSpec};
use crate::HookResult;
use crate::state::HookState;

pub struct HookRunner {
    state_dir: PathBuf,
    force: bool,
}

impl HookRunner {
    pub fn new(state_dir: PathBuf) -> Self {
        Self { state_dir, force: false }
    }

    pub fn with_force(state_dir: PathBuf) -> Self {
        Self { state_dir, force: true }
    }

    pub fn run_hook(&self, spec: &HookSpec) -> HookResult {
        let mut state = HookState::load(&self.state_dir).unwrap_or_default();

        if spec.phase == HookPhase::Setup && !self.force && state.is_done(&spec.name) {
            return HookResult::Skipped;
        }

        let result = Command::new("nu")
            .arg(&spec.script)
            .status();

        match result {
            Ok(status) if status.success() => {
                if spec.phase == HookPhase::Setup {
                    state.mark_done(&spec.name);
                    let _ = state.save(&self.state_dir);
                }
                HookResult::Ok
            }
            Ok(status) => HookResult::Failed(format!("exit code {}", status.code().unwrap_or(-1))),
            Err(e) => HookResult::Failed(e.to_string()),
        }
    }
}
