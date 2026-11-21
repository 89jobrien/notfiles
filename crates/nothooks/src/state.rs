use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Which setup hooks have completed. Persistence lives behind `ports::StateStore`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HookState {
    pub completed_setup_hooks: HashSet<String>,
}

impl HookState {
    pub fn mark_done(&mut self, name: &str) {
        self.completed_setup_hooks.insert(name.to_string());
    }

    pub fn is_done(&self, name: &str) -> bool {
        self.completed_setup_hooks.contains(name)
    }
}
