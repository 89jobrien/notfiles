use crate::HookResult;
use crate::state::HookState;
use notcore::{HookPhase, HookSpec};
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

    pub fn run_hook(&self, spec: &HookSpec) -> HookResult {
        let mut state = HookState::load(&self.state_dir).unwrap_or_default();

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
                    let _ = state.save(&self.state_dir);
                }
                HookResult::Ok
            }
            Ok(status) => HookResult::Failed(format!("exit code {}", status.code().unwrap_or(-1))),
            Err(e) => HookResult::Failed(e.to_string()),
        }
    }
}
