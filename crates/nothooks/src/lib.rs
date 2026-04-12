pub mod runner;
pub mod state;

use notcore::{HookPhase, HookSpec, Report};
pub use runner::HookRunner;

#[derive(Debug, PartialEq)]
pub enum HookResult {
    Ok,
    Skipped,
    Failed(String),
}

/// Run all hooks matching `phase` and collect into a `Report`.
///
/// State is loaded once and saved once per call — not once per hook.
pub fn run_phase(hooks: &[HookSpec], phase: &HookPhase, runner: &HookRunner) -> Report {
    runner.run_phase(hooks, phase)
}
