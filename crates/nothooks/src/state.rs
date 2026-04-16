use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

const STATE_FILE: &str = ".nothooks-state.toml";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct HookState {
    pub completed_setup_hooks: HashSet<String>,
}

impl HookState {
    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join(STATE_FILE);
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        let path = dir.join(STATE_FILE);
        let tmp = dir.join(format!("{STATE_FILE}.tmp"));
        std::fs::write(&tmp, toml::to_string(self)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn mark_done(&mut self, name: &str) {
        self.completed_setup_hooks.insert(name.to_string());
    }

    pub fn is_done(&self, name: &str) -> bool {
        self.completed_setup_hooks.contains(name)
    }
}
