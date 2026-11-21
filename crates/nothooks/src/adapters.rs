use crate::ports::{ScriptExit, ScriptInvocation, ScriptRunner, StateStore};
use crate::state::HookState;
use anyhow::Result;
use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

const STATE_FILE: &str = ".nothooks-state.toml";

/// Launches hooks as real child processes.
pub struct ProcessRunner;

impl ScriptRunner for ProcessRunner {
    fn run(&self, invocation: &ScriptInvocation<'_>) -> io::Result<ScriptExit> {
        // Interpreter and script come from config and are passed as discrete args with no
        // shell involved, so argument injection is not possible.
        let status = Command::new(invocation.interpreter)
            .arg(invocation.script)
            .envs(invocation.env.iter().map(|(k, v)| (k, v)))
            .status()?;
        Ok(ScriptExit {
            code: status.code(),
        })
    }
}

/// Persists state as `.nothooks-state.toml` inside a directory, writing atomically.
pub struct FsStateStore {
    dir: PathBuf,
}

impl FsStateStore {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }
}

impl StateStore for FsStateStore {
    fn load(&self) -> Result<HookState> {
        let path = self.dir.join(STATE_FILE);
        if !path.exists() {
            return Ok(HookState::default());
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(toml::from_str(&content)?)
    }

    fn save(&self, state: &HookState) -> Result<()> {
        let path = self.dir.join(STATE_FILE);
        let tmp = self.dir.join(format!("{STATE_FILE}.tmp"));
        std::fs::write(&tmp, toml::to_string(state)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

#[derive(Default)]
struct Inner {
    state: HookState,
    loads: usize,
    saves: usize,
}

/// Test double for `StateStore`. Clones share state, so a test can keep a handle
/// after moving one into a `HookRunner`.
#[derive(Clone, Default)]
pub struct InMemoryStateStore {
    inner: Rc<RefCell<Inner>>,
}

impl InMemoryStateStore {
    pub fn is_done(&self, name: &str) -> bool {
        self.inner.borrow().state.is_done(name)
    }

    pub fn loads(&self) -> usize {
        self.inner.borrow().loads
    }

    pub fn saves(&self) -> usize {
        self.inner.borrow().saves
    }
}

impl StateStore for InMemoryStateStore {
    fn load(&self) -> Result<HookState> {
        let mut inner = self.inner.borrow_mut();
        inner.loads += 1;
        Ok(inner.state.clone())
    }

    fn save(&self, state: &HookState) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.saves += 1;
        inner.state = state.clone();
        Ok(())
    }
}
