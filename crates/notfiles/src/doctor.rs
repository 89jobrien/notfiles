use std::path::Path;

use crate::linker::State;
use crate::status::{self, FileStatus};
use notcore::Config;

use crate::ports::FileStore;

/// A single problem surfaced by `notfiles doctor`.
#[derive(Debug, PartialEq)]
pub enum DoctorIssue {
    /// A package file is symlinked to something other than its source, or a
    /// non-managed file already occupies the target path.
    Conflict { package: String, target: String },
    /// A package file that should be linked/copied is not present at the target.
    Missing { package: String, target: String },
    /// A tracked state entry whose source file no longer exists.
    Orphan { package: String, target: String },
}

#[derive(Debug, Default, PartialEq)]
pub struct DoctorReport {
    pub issues: Vec<DoctorIssue>,
}

impl DoctorReport {
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Run all doctor checks across every package and aggregate the results.
pub fn run(
    dotfiles_dir: &Path,
    config: &Config,
    state: &State,
    fs: &dyn FileStore,
) -> DoctorReport {
    let mut issues = Vec::new();
    collect_link_state_issues(dotfiles_dir, config, state, fs, &mut issues);
    DoctorReport { issues }
}

fn collect_link_state_issues(
    dotfiles_dir: &Path,
    config: &Config,
    state: &State,
    fs: &dyn FileStore,
    issues: &mut Vec<DoctorIssue>,
) {
    let packages = crate::package::discover_packages_filtered_with_store(dotfiles_dir, config, fs)
        .unwrap_or_default();

    for package in &packages {
        let entries = status::package_status(dotfiles_dir, config, state, package, fs);
        for entry in entries {
            let target = entry.target.to_string_lossy().to_string();
            match entry.status {
                FileStatus::Conflict => issues.push(DoctorIssue::Conflict {
                    package: package.clone(),
                    target,
                }),
                FileStatus::Missing => issues.push(DoctorIssue::Missing {
                    package: package.clone(),
                    target,
                }),
                FileStatus::Orphan => issues.push(DoctorIssue::Orphan {
                    package: package.clone(),
                    target,
                }),
                FileStatus::Linked | FileStatus::Copied => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::memory::InMemoryFileStore;
    use notcore::Method;

    #[test]
    fn aggregates_missing_conflict_orphan_across_packages() {
        let fs = InMemoryFileStore::new();
        let dotfiles_dir = Path::new("/dotfiles");

        // package "zsh": one file, never linked -> Missing
        fs.add_file(dotfiles_dir.join("zsh/.zshrc"), b"# zshrc");

        // package "git": one file, linked to the wrong place -> Conflict
        let home = dirs::home_dir().expect("home dir must resolve in test env");
        fs.add_file(dotfiles_dir.join("git/.gitconfig"), b"[user]");
        fs.add_file(home.join(".gitconfig"), b"stale");

        let config = Config::default();

        // "git" needs to be discoverable — discover_packages_filtered_with_store walks
        // dotfiles_dir, so both package dirs must exist via add_file above.
        let mut state = State::default();
        // Orphan: tracked state entry whose source no longer exists.
        state.entries.push(crate::linker::StateEntry {
            package: "git".to_string(),
            source: dotfiles_dir.join("git/.gone").to_string_lossy().to_string(),
            target: home.join(".gone").to_string_lossy().to_string(),
            method: Method::Symlink,
            linked_at: "2026-01-01T00:00:00Z".to_string(),
        });

        let report = run(dotfiles_dir, &config, &state, &fs);

        assert!(
            report
                .issues
                .iter()
                .any(|i| matches!(i, DoctorIssue::Missing { package, .. } if package == "zsh"))
        );
        assert!(
            report
                .issues
                .iter()
                .any(|i| matches!(i, DoctorIssue::Conflict { package, .. } if package == "git"))
        );
        assert!(
            report
                .issues
                .iter()
                .any(|i| matches!(i, DoctorIssue::Orphan { package, .. } if package == "git"))
        );
        assert!(!report.is_clean());
    }

    #[test]
    fn clean_report_when_nothing_to_flag() {
        let fs = InMemoryFileStore::new();
        let dotfiles_dir = Path::new("/dotfiles");
        // No packages at all -> nothing to check, report is clean.
        fs.add_dir(dotfiles_dir);

        let config = Config::default();
        let state = State::default();

        let report = run(dotfiles_dir, &config, &state, &fs);
        assert!(report.is_clean());
    }
}
