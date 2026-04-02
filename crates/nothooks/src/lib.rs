pub mod runner;
pub mod state;

use notcore::{HookPhase, HookSpec, Report, StepStatus};
pub use runner::HookRunner;

#[derive(Debug, PartialEq)]
pub enum HookResult {
    Ok,
    Skipped,
    Failed(String),
}

/// Run all hooks matching `phase` and collect into a `Report`.
pub fn run_phase(hooks: &[HookSpec], phase: &HookPhase, runner: &HookRunner) -> Report {
    let mut report = Report::default();
    for hook in hooks.iter().filter(|h| &h.phase == phase) {
        let result = runner.run_hook(hook);
        let status = match &result {
            HookResult::Ok => StepStatus::Ok,
            HookResult::Skipped => StepStatus::Skipped,
            HookResult::Failed(msg) => StepStatus::Failed(msg.clone()),
        };
        report.add(&hook.name, status);
    }
    report
}
