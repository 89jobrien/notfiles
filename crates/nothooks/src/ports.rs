use crate::state::HookState;
use anyhow::Result;
use std::io;
use std::path::Path;

/// One hook process to launch: `interpreter script`, with `env` added to the inherited environment.
///
/// Later entries in `env` override earlier ones with the same key.
pub struct ScriptInvocation<'a> {
    pub interpreter: &'a str,
    pub script: &'a Path,
    pub env: &'a [(String, String)],
}

/// How a hook process ended. `code` is `None` when it was killed by a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptExit {
    pub code: Option<i32>,
}

impl ScriptExit {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Boundary for launching hook scripts.
///
/// Contract: `Err` means the process could not be started (missing interpreter, bad path);
/// a script that ran and failed is `Ok` with a non-zero `ScriptExit`.
pub trait ScriptRunner {
    fn run(&self, invocation: &ScriptInvocation<'_>) -> io::Result<ScriptExit>;
}

/// Boundary for persisting which setup hooks have completed.
///
/// Contract: `load` on a store that was never saved returns an empty `HookState`, and
/// `load` after `save(s)` returns a state equal to `s`.
pub trait StateStore {
    fn load(&self) -> Result<HookState>;
    fn save(&self, state: &HookState) -> Result<()>;
}
