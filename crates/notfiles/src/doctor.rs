use std::path::Path;

use serde_json::json;

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
    /// The dotfiles git repo has uncommitted changes.
    DirtyGit,
    /// A directory exists in the dotfiles dir but isn't in `notfiles.toml`'s
    /// `include` list, so it's silently ignored.
    UnlistedPackage { package: String },
    /// A package named in `notfiles.toml`'s `include` list has no matching
    /// directory on disk.
    MissingPackage { package: String },
}

/// Abstracts the git-status check so it can be faked in tests, matching the
/// `FileStore` dependency-injection pattern used elsewhere in this crate.
pub trait GitStatus {
    /// Returns `Some(true)` if the repo has uncommitted changes, `Some(false)`
    /// if clean, or `None` if the status could not be determined (no `.git`,
    /// `git` not on PATH, etc.) — which is not treated as an issue.
    fn is_dirty(&self, repo_dir: &Path) -> Option<bool>;
}

pub struct SystemGitStatus;

impl GitStatus for SystemGitStatus {
    fn is_dirty(&self, repo_dir: &Path) -> Option<bool> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_dir)
            .args(["status", "--porcelain"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(!output.stdout.is_empty())
    }
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

/// Run all doctor checks across every package and aggregate the results,
/// using the real system `git` for the dirty-repo check.
pub fn run(
    dotfiles_dir: &Path,
    config: &Config,
    state: &State,
    fs: &dyn FileStore,
) -> DoctorReport {
    run_with_git(dotfiles_dir, config, state, fs, &SystemGitStatus)
}

/// Run all doctor checks with an injectable `GitStatus` (for testing).
pub fn run_with_git(
    dotfiles_dir: &Path,
    config: &Config,
    state: &State,
    fs: &dyn FileStore,
    git: &dyn GitStatus,
) -> DoctorReport {
    let mut issues = Vec::new();
    collect_link_state_issues(dotfiles_dir, config, state, fs, &mut issues);
    if git.is_dirty(dotfiles_dir) == Some(true) {
        issues.push(DoctorIssue::DirtyGit);
    }
    collect_package_mismatch_issues(dotfiles_dir, config, fs, &mut issues);
    DoctorReport { issues }
}

/// Flags directories on disk that aren't in an explicit `include` list, and
/// `include` entries with no matching directory on disk. No-op when the
/// config has no `include` list (all discovered packages are implicitly in
/// scope).
fn collect_package_mismatch_issues(
    dotfiles_dir: &Path,
    config: &Config,
    fs: &dyn FileStore,
    issues: &mut Vec<DoctorIssue>,
) {
    let Some(included) = config.included_packages() else {
        return;
    };

    let on_disk =
        crate::package::discover_packages_with_store(dotfiles_dir, fs).unwrap_or_default();

    for package in &on_disk {
        if !included.contains(&package.as_str()) {
            issues.push(DoctorIssue::UnlistedPackage {
                package: package.clone(),
            });
        }
    }
    for package in &included {
        if !on_disk.iter().any(|p| p == package) {
            issues.push(DoctorIssue::MissingPackage {
                package: package.to_string(),
            });
        }
    }
}

fn issue_kind(issue: &DoctorIssue) -> &'static str {
    match issue {
        DoctorIssue::Conflict { .. } => "conflict",
        DoctorIssue::Missing { .. } => "missing",
        DoctorIssue::Orphan { .. } => "orphan",
        DoctorIssue::DirtyGit => "dirty_git",
        DoctorIssue::UnlistedPackage { .. } => "unlisted_package",
        DoctorIssue::MissingPackage { .. } => "missing_package",
    }
}

fn issue_line(issue: &DoctorIssue) -> String {
    match issue {
        DoctorIssue::Conflict { package, target } => {
            format!("conflict   {package}: {target}")
        }
        DoctorIssue::Missing { package, target } => {
            format!("missing    {package}: {target}")
        }
        DoctorIssue::Orphan { package, target } => {
            format!("orphan     {package}: {target}")
        }
        DoctorIssue::DirtyGit => "dirty git  dotfiles repo has uncommitted changes".to_string(),
        DoctorIssue::UnlistedPackage { package } => {
            format!("unlisted   {package}: on disk but not in notfiles.toml include list")
        }
        DoctorIssue::MissingPackage { package } => {
            format!("missing    {package}: in notfiles.toml include list but no directory on disk")
        }
    }
}

/// Human-readable summary: a "clean" line, or one line per issue.
pub fn format_report(report: &DoctorReport) -> String {
    if report.is_clean() {
        return "\x1b[32mnotfiles doctor: clean, no issues found.\x1b[0m".to_string();
    }
    let mut out = format!(
        "\x1b[33m{} issue{} found:\x1b[0m\n",
        report.issues.len(),
        if report.issues.len() == 1 { "" } else { "s" }
    );
    for issue in &report.issues {
        out.push_str("  ");
        out.push_str(&issue_line(issue));
        out.push('\n');
    }
    out.trim_end().to_string()
}

pub fn print_report(report: &DoctorReport) {
    println!("{}", format_report(report));
}

pub fn print_report_json(report: &DoctorReport) {
    let items: Vec<_> = report
        .issues
        .iter()
        .map(|issue| json!({"kind": issue_kind(issue), "detail": issue_line(issue)}))
        .collect();
    let obj = json!({"clean": report.is_clean(), "issues": items});
    println!("{}", serde_json::to_string(&obj).unwrap_or_default());
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

    struct FakeGitStatus(Option<bool>);

    impl GitStatus for FakeGitStatus {
        fn is_dirty(&self, _repo_dir: &Path) -> Option<bool> {
            self.0
        }
    }

    #[test]
    fn flags_dirty_git_state() {
        let fs = InMemoryFileStore::new();
        let dotfiles_dir = Path::new("/dotfiles");
        fs.add_dir(dotfiles_dir);

        let config = Config::default();
        let state = State::default();

        let dirty = run_with_git(
            dotfiles_dir,
            &config,
            &state,
            &fs,
            &FakeGitStatus(Some(true)),
        );
        assert!(dirty.issues.contains(&DoctorIssue::DirtyGit));

        let clean = run_with_git(
            dotfiles_dir,
            &config,
            &state,
            &fs,
            &FakeGitStatus(Some(false)),
        );
        assert!(!clean.issues.contains(&DoctorIssue::DirtyGit));

        // Unknown git state (no .git, git missing, etc.) is not an issue.
        let unknown = run_with_git(dotfiles_dir, &config, &state, &fs, &FakeGitStatus(None));
        assert!(!unknown.issues.contains(&DoctorIssue::DirtyGit));
    }

    #[test]
    fn flags_unlisted_and_missing_packages() {
        let fs = InMemoryFileStore::new();
        let dotfiles_dir = Path::new("/dotfiles");
        // "zsh" is included and present, "extra" is present but not included,
        // "ssh" is included but has no directory on disk.
        fs.add_dir(dotfiles_dir.join("zsh"));
        fs.add_dir(dotfiles_dir.join("extra"));

        let toml_str = r#"
[defaults]
target = "~"
include = ["zsh", "ssh"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let state = State::default();

        let report = run_with_git(dotfiles_dir, &config, &state, &fs, &FakeGitStatus(None));

        assert!(report.issues.contains(&DoctorIssue::UnlistedPackage {
            package: "extra".to_string(),
        }));
        assert!(report.issues.contains(&DoctorIssue::MissingPackage {
            package: "ssh".to_string(),
        }));
        assert!(
            !report
                .issues
                .iter()
                .any(|i| matches!(i, DoctorIssue::UnlistedPackage { package } if package == "zsh"))
        );
    }

    #[test]
    fn no_mismatch_issues_without_include_list() {
        let fs = InMemoryFileStore::new();
        let dotfiles_dir = Path::new("/dotfiles");
        fs.add_dir(dotfiles_dir.join("anything"));

        let config = Config::default();
        let state = State::default();

        let report = run_with_git(dotfiles_dir, &config, &state, &fs, &FakeGitStatus(None));
        assert!(
            !report
                .issues
                .iter()
                .any(|i| matches!(i, DoctorIssue::UnlistedPackage { .. }))
        );
    }

    #[test]
    fn doctor_report_prints_human_summary() {
        let clean = DoctorReport::default();
        assert!(format_report(&clean).contains("clean"));

        let dirty = DoctorReport {
            issues: vec![
                DoctorIssue::Missing {
                    package: "zsh".to_string(),
                    target: "/home/.zshrc".to_string(),
                },
                DoctorIssue::DirtyGit,
            ],
        };
        let summary = format_report(&dirty);
        assert!(summary.contains("2 issues found"));
        assert!(summary.contains("zsh"));
        assert!(summary.contains("dirty git"));
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
