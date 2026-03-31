use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookPhase {
    Dot,
    Setup,
}

impl std::fmt::Display for HookPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookPhase::Dot => write!(f, "dot"),
            HookPhase::Setup => write!(f, "setup"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSpec {
    pub name: String,
    pub script: String,
    pub phase: HookPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSpec {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepStatus {
    Ok,
    Skipped,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct Step {
    pub name: String,
    pub status: StepStatus,
}

#[derive(Debug, Default)]
pub struct Report {
    pub steps: Vec<Step>,
}

impl Report {
    pub fn add(&mut self, name: impl Into<String>, status: StepStatus) {
        self.steps.push(Step { name: name.into(), status });
    }

    pub fn print(&self) {
        for step in &self.steps {
            let icon = match &step.status {
                StepStatus::Ok => "\x1b[32m✓\x1b[0m",
                StepStatus::Skipped => "\x1b[33m-\x1b[0m",
                StepStatus::Failed(_) => "\x1b[31m✗\x1b[0m",
            };
            let detail = match &step.status {
                StepStatus::Failed(msg) => format!(" ({msg})"),
                _ => String::new(),
            };
            println!("{icon} {}{detail}", step.name);
        }
    }

    pub fn has_failures(&self) -> bool {
        self.steps.iter().any(|s| matches!(s.status, StepStatus::Failed(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_has_failures() {
        let mut r = Report::default();
        r.add("step1", StepStatus::Ok);
        assert!(!r.has_failures());
        r.add("step2", StepStatus::Failed("oops".into()));
        assert!(r.has_failures());
    }

    #[test]
    fn test_hook_phase_display() {
        assert_eq!(HookPhase::Dot.to_string(), "dot");
        assert_eq!(HookPhase::Setup.to_string(), "setup");
    }
}
