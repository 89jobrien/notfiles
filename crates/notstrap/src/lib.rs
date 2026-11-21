use anyhow::{Context, Result};
use notcore::{HookPhase, Report, StepStatus};
use notfiles::{LinkOptions, link};
use nothooks::{HookRunner, run_phase};
use notsecrets::sources::IdentitySource;
use notsecrets::{
    BitwardenSource, FileSource, PromptSource, SecretResolver, YubikeySource, resolve_identities,
};
use prompt::Prompter;
use serde::Deserialize;
use std::path::PathBuf;

pub mod prereqs;
pub mod prompt;
pub mod repo;
pub mod tools;
pub mod wizard;

pub use notnet::TailscaleOptions;

#[derive(Deserialize)]
pub struct NotstrapConfig {
    pub bootstrap: BootstrapSection,
    /// When present, notstrap joins the tailnet before cloning dotfiles.
    pub tailscale: Option<TailscaleOptions>,
    #[serde(default)]
    pub hooks: Vec<notcore::HookSpec>,
    #[serde(default)]
    pub prereqs: PrereqsSection,
}

/// `[prereqs]`: commands that must exist before bootstrap starts.
#[derive(Deserialize)]
pub struct PrereqsSection {
    #[serde(default = "prereqs::default_required")]
    pub required: Vec<String>,
}

impl Default for PrereqsSection {
    fn default() -> Self {
        Self {
            required: prereqs::default_required(),
        }
    }
}

#[derive(Deserialize)]
pub struct BootstrapSection {
    /// Required when `[tailscale]` is absent. When `[tailscale]` is present,
    /// `gitea_url` from that section is used as the clone URL instead.
    pub dotfiles_repo: Option<String>,
    pub dotfiles_dir: String,
    #[serde(default = "default_bw_item")]
    pub bw_age_item: String,
    #[serde(default = "default_sops_file")]
    pub sops_file: String,
}

pub(crate) fn default_bw_item() -> String {
    "age-key-dotfiles".to_string()
}
pub(crate) fn default_sops_file() -> String {
    "secrets/bootstrap.sops.env".to_string()
}

/// Checks that the required commands exist; receives the `[prereqs]` list.
pub type PrereqCheck = Box<dyn Fn(&[String]) -> Result<()>>;

/// Reconciles declared tools for a dotfiles checkout; `None` means no `[doctor]` table.
pub type ToolsReconciler = Box<dyn Fn(&std::path::Path) -> Result<Option<tools::ToolsSummary>>>;

pub struct BootstrapOptions {
    pub config: PathBuf,
    pub force: bool,
    pub key_file: Option<PathBuf>,
    pub dotfiles: Option<PathBuf>,
    /// Override the Tailscale options from the config file. `None` here defers to the
    /// config; use `Some(None)` to explicitly skip Tailscale even when the config has
    /// a `[tailscale]` section (useful in tests and CI).
    ///
    /// Encoding: `None` = use config, `Some(None)` = skip, `Some(Some(opts))` = override.
    pub tailscale: Option<Option<TailscaleOptions>>,
    /// None = skip prereq check (tests). Some(f) = run f with the required commands
    /// from `[prereqs]` (or the defaults).
    pub check_prereqs: Option<PrereqCheck>,
    /// None = skip tool reconciliation (tests). Some(f) = run f against the dotfiles
    /// checkout after hooks; it returns `None` when there is no `[doctor]` table.
    pub reconcile_tools: Option<ToolsReconciler>,
    /// How the dotfiles repository is fetched. Production uses `repo::SystemGit`.
    pub git: Box<dyn repo::GitClone>,
    /// Some = run the interactive wizard before anything else. None = no prompts.
    pub prompter: Option<Box<dyn Prompter>>,
    /// `--yes`: tell hooks not to prompt (exported as `NOTSTRAP_YES=1`).
    pub assume_yes: bool,
    /// Path to notsecrets.toml. None = skip secret resolution.
    pub secrets_config: Option<PathBuf>,
}

pub fn run(opts: BootstrapOptions) -> Result<Report> {
    let mut report = Report::default();

    // 1. Load config (read-only; needed for the `[prereqs]` list)
    let config_content = std::fs::read_to_string(&opts.config)
        .with_context(|| format!("cannot read {}", opts.config.display()))?;
    let mut cfg: NotstrapConfig = toml::from_str(&config_content)?;

    // Wizard: confirm or adjust the plan before anything happens
    if let Some(prompter) = &opts.prompter {
        match wizard::run(&cfg, prompter.as_ref()) {
            Ok(Some(choices)) => {
                wizard::apply(&mut cfg, choices);
                report.add("wizard", StepStatus::Ok);
            }
            Ok(None) => {
                report.add("wizard", StepStatus::Failed("cancelled by user".into()));
                return Ok(report);
            }
            Err(e) => {
                report.add("wizard", StepStatus::Failed(e.to_string()));
                return Ok(report);
            }
        }
    }

    // 2. Prerequisites
    if let Some(check) = opts.check_prereqs {
        match check(&cfg.prereqs.required) {
            Ok(_) => {
                report.add("prerequisites", StepStatus::Ok);
            }
            Err(e) => {
                report.add("prerequisites", StepStatus::Failed(e.to_string()));
                return Ok(report);
            }
        }
    }

    let dotfiles_dir = match opts.dotfiles {
        Some(d) => d,
        None => notcore::expand_tilde(&cfg.bootstrap.dotfiles_dir).with_context(|| {
            format!("cannot expand dotfiles_dir: {}", cfg.bootstrap.dotfiles_dir)
        })?,
    };

    // Resolve which Tailscale options to use: CLI override > config > None.
    let ts_opts: Option<TailscaleOptions> = match opts.tailscale {
        Some(override_val) => override_val, // Some(None) = skip, Some(Some(o)) = use override
        None => cfg.tailscale.clone(),      // defer to config
    };

    // Resolve dotfiles clone URL: Tailscale gitea_url takes precedence over dotfiles_repo.
    let clone_url: Option<String> = ts_opts
        .as_ref()
        .map(|ts| ts.gitea_url.clone())
        .or_else(|| cfg.bootstrap.dotfiles_repo.clone());

    // 3. Tailscale (inserted before clone, skipped when ts_opts is None)
    if let Some(ref ts) = ts_opts {
        match notnet::ensure_connected(ts) {
            Ok(true) => report.add("tailscale", StepStatus::Ok),
            Ok(false) => report.add("tailscale", StepStatus::Skipped),
            Err(e) => {
                report.add("tailscale", StepStatus::Failed(e.to_string()));
                return Ok(report);
            }
        }
    }

    // 4. Clone dotfiles if missing
    let clone_url = clone_url.ok_or_else(|| {
        anyhow::anyhow!(
            "no dotfiles clone URL: set [bootstrap].dotfiles_repo or add a [tailscale] section"
        )
    })?;
    match repo::clone_if_missing(opts.git.as_ref(), &clone_url, &dotfiles_dir) {
        Ok(true) => {
            report.add("clone dotfiles", StepStatus::Ok);
        }
        Ok(false) => {
            report.add("clone dotfiles", StepStatus::Skipped);
        }
        Err(e) => {
            report.add("clone dotfiles", StepStatus::Failed(e.to_string()));
            return Ok(report);
        }
    }

    // 5. Resolve age identities
    let sources: Vec<Box<dyn IdentitySource>> = if let Some(kf) = opts.key_file {
        vec![Box::new(FileSource::new(kf))]
    } else {
        vec![
            Box::new(YubikeySource),
            Box::new(BitwardenSource::new(&cfg.bootstrap.bw_age_item)),
            Box::new(PromptSource),
        ]
    };

    let _identities = match resolve_identities(sources) {
        Ok(ids) => {
            report.add("age key", StepStatus::Ok);
            ids
        }
        Err(e) => {
            report.add("age key", StepStatus::Failed(e.to_string()));
            return Ok(report);
        }
    };

    // 5b. Resolve secrets via notsecrets.toml (optional). They go to hooks as environment
    // variables; the bootstrap process itself is never mutated.
    let secret_env = match opts.secrets_config {
        Some(path) => match resolve_secret_env(&path) {
            Ok(vars) => {
                report.add("secrets", StepStatus::Ok);
                vars
            }
            Err(e) => {
                report.add("secrets", StepStatus::Failed(e.to_string()));
                return Ok(report);
            }
        },
        None => Vec::new(),
    };

    // 6. Link dotfiles
    let link_opts = LinkOptions {
        force: opts.force,
        no_backup: false,
        dry_run: false,
        verbose: false,
    };
    match link(&dotfiles_dir, &[], &link_opts) {
        Ok(state) => {
            let count = state.entries.len();
            report.add(format!("link dotfiles ({count} files)"), StepStatus::Ok);
        }
        Err(e) => {
            report.add("link dotfiles", StepStatus::Failed(e.to_string()));
            return Ok(report);
        }
    }

    // 7. Run hooks
    let mut hook_env = secret_env;
    if opts.assume_yes {
        hook_env.push(("NOTSTRAP_YES".to_string(), "1".to_string()));
    }
    let runner = if opts.force {
        HookRunner::with_force(dotfiles_dir.clone())
    } else {
        HookRunner::new(dotfiles_dir.clone())
    }
    .with_env(hook_env);

    for (phase, label) in [
        (HookPhase::Dot, "dot hooks"),
        (HookPhase::Setup, "setup hooks"),
    ] {
        let phase_report = match run_phase(&cfg.hooks, &phase, &runner) {
            Ok(report) => report,
            Err(e) => {
                report.add(label, StepStatus::Failed(e.to_string()));
                return Ok(report);
            }
        };
        let failed = phase_report
            .steps
            .iter()
            .filter(|s| matches!(s.status, notcore::StepStatus::Failed(_)))
            .count();
        let summary = if failed > 0 {
            StepStatus::Failed(format!("{failed} failed"))
        } else {
            StepStatus::Ok
        };
        report.add(label, summary);
        if failed > 0 {
            return Ok(report);
        }
    }

    // 8. Reconcile tools declared by the dotfiles manifest
    if let Some(reconcile) = opts.reconcile_tools {
        match reconcile(&dotfiles_dir) {
            Ok(None) => report.add("tools", StepStatus::Skipped),
            Ok(Some(summary)) => add_tools_step(&mut report, &summary),
            Err(e) => report.add("tools", StepStatus::Failed(e.to_string())),
        }
    }

    Ok(report)
}

/// Turn a tool reconciliation summary into one report step.
fn add_tools_step(report: &mut Report, summary: &tools::ToolsSummary) {
    let installed = summary.installed.len();
    let note = if summary.undeclared.is_empty() {
        String::new()
    } else {
        format!("; declared nowhere: {}", summary.undeclared.join(", "))
    };
    if summary.unresolved.is_empty() {
        report.add(
            format!("tools ({installed} install step(s){note})"),
            StepStatus::Ok,
        );
    } else {
        report.add(
            "tools",
            StepStatus::Failed(format!(
                "unresolved: {}{note}",
                summary.unresolved.join(", ")
            )),
        );
    }
}

/// Strip outer quotes from an env value and unescape inner escape sequences.
///
/// - `"val\"ue"` → `val"ue`  (double-quoted, with backslash escapes processed)
/// - `'value'`   → `value`   (single-quoted, no escape processing)
/// - `value`     → `value`   (unquoted, returned as-is)
fn unquote_env_value(v: &str) -> String {
    if v.len() >= 2 {
        if v.starts_with('"') && v.ends_with('"') {
            let inner = &v[1..v.len() - 1];
            // Process backslash escapes within double-quoted strings.
            let mut result = String::with_capacity(inner.len());
            let mut chars = inner.chars();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    match chars.next() {
                        Some('"') => result.push('"'),
                        Some('\\') => result.push('\\'),
                        Some('n') => result.push('\n'),
                        Some('t') => result.push('\t'),
                        Some(other) => {
                            result.push('\\');
                            result.push(other);
                        }
                        None => result.push('\\'),
                    }
                } else {
                    result.push(c);
                }
            }
            return result;
        }
        if v.starts_with('\'') && v.ends_with('\'') {
            return v[1..v.len() - 1].to_string();
        }
    }
    v.to_string()
}

/// Load `notsecrets.toml` and resolve every secret to validated `(key, value)` pairs.
fn resolve_secret_env(path: &std::path::Path) -> Result<Vec<(String, String)>> {
    let config = notsecrets::load_config(path)?;
    let resolver = SecretResolver::from_config(config)?;
    validated_env(resolver.resolve_all()?)
}

/// Validate every entry with [`parse_env_line`]; one rejected entry fails the whole set.
/// Output is sorted by key so hook environments are deterministic.
fn validated_env(vars: std::collections::HashMap<String, String>) -> Result<Vec<(String, String)>> {
    let mut out = Vec::with_capacity(vars.len());
    for (key, value) in &vars {
        if let Some(pair) = parse_env_line(&format!("{key}={value}"))? {
            out.push(pair);
        }
    }
    out.sort();
    Ok(out)
}

/// Parse and validate a single env line (`KEY=value`), returning `(key, value)` if safe to inject.
/// Returns `Ok(None)` for blank lines and comments.
/// Returns `Err` for null bytes or dangerous keys.
pub fn parse_env_line(line: &str) -> Result<Option<(String, String)>> {
    let Some((k, v)) = line.split_once('=') else {
        return Ok(None);
    };
    let k = k.trim().to_string();
    let v = unquote_env_value(v.trim());

    if k.is_empty() || k.starts_with('#') {
        return Ok(None);
    }

    // Issue #4: reject null bytes
    if k.contains('\0') || v.contains('\0') {
        anyhow::bail!("env key or value contains null byte: {:?}", k);
    }

    // Issue #5: block dangerous keys that affect dynamic linker / shell
    const BLOCKED: &[&str] = &[
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "DYLD_INSERT_LIBRARIES",
        "DYLD_LIBRARY_PATH",
        "IFS",
        "PATH",
    ];
    if BLOCKED.contains(&k.as_str()) {
        anyhow::bail!("refusing to inject sensitive env key: {k}");
    }

    Ok(Some((k, v)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn parse(extra: &str) -> NotstrapConfig {
        let text = format!("[bootstrap]\ndotfiles_dir = \"~/x\"\n{extra}");
        toml::from_str(&text).expect("from_str should succeed")
    }

    #[test]
    fn prereqs_default_when_table_absent() {
        assert_eq!(parse("").prereqs.required, ["nu", "sops", "age"]);
    }

    #[test]
    fn prereqs_read_from_table() {
        let cfg = parse("[prereqs]\nrequired = [\"nu\", \"mise\"]\n");
        assert_eq!(cfg.prereqs.required, ["nu", "mise"]);
    }

    #[test]
    fn run_passes_configured_prereqs_to_the_check() {
        let dir = tempfile::tempdir().expect("tempdir should succeed");
        let config = dir.path().join("notstrap.toml");
        std::fs::write(
            &config,
            "[bootstrap]\ndotfiles_dir = \"/nowhere\"\n[prereqs]\nrequired = [\"foo\", \"bar\"]\n",
        )
        .expect("write should succeed");
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        let opts = BootstrapOptions {
            config,
            force: false,
            key_file: None,
            dotfiles: None,
            tailscale: Some(None),
            check_prereqs: Some(Box::new(move |req| {
                *sink.borrow_mut() = req.to_vec();
                anyhow::bail!("stop here")
            })),
            reconcile_tools: None,
            git: Box::new(repo::FakeGit::default()),
            prompter: None,
            assume_yes: false,
            secrets_config: None,
        };
        let report = run(opts).expect("run should succeed");
        assert_eq!(*seen.borrow(), ["foo", "bar"]);
        assert!(report.has_failures());
    }

    #[test]
    fn tools_step_ok_notes_undeclared_targets() {
        let mut report = Report::default();
        let summary = tools::ToolsSummary {
            installed: vec!["mise install".into()],
            unresolved: vec![],
            undeclared: vec!["zb".into(), "pj".into()],
        };
        add_tools_step(&mut report, &summary);
        assert!(!report.has_failures());
        assert!(report.steps[0].name.contains("1 install step(s)"));
        assert!(report.steps[0].name.contains("declared nowhere: zb, pj"));
    }

    #[test]
    fn tools_step_fails_when_declared_tools_stay_missing() {
        let mut report = Report::default();
        let summary = tools::ToolsSummary {
            installed: vec![],
            unresolved: vec!["colima".into()],
            undeclared: vec![],
        };
        add_tools_step(&mut report, &summary);
        assert!(report.has_failures());
    }

    /// Issue #4: null byte in key must be rejected.
    #[test]
    fn parse_env_line_rejects_null_in_key() {
        let result = parse_env_line("KEY\0EVIL=value");
        assert!(result.is_err(), "expected error for null byte in key");
        assert!(result.unwrap_err().to_string().contains("null byte"));
    }

    /// Issue #4: null byte in value must be rejected.
    #[test]
    fn parse_env_line_rejects_null_in_value() {
        let result = parse_env_line("KEY=val\0ue");
        assert!(result.is_err(), "expected error for null byte in value");
        assert!(result.unwrap_err().to_string().contains("null byte"));
    }

    /// Issue #5: LD_PRELOAD must be blocked.
    #[test]
    fn parse_env_line_blocks_ld_preload() {
        let result = parse_env_line("LD_PRELOAD=/evil/lib.so");
        assert!(result.is_err(), "expected error for LD_PRELOAD");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("sensitive env key")
        );
    }

    /// Issue #5: PATH must be blocked.
    #[test]
    fn parse_env_line_blocks_path() {
        let result = parse_env_line("PATH=/attacker/bin:/usr/bin");
        assert!(result.is_err(), "expected error for PATH");
    }

    /// Issue #5: DYLD_INSERT_LIBRARIES must be blocked.
    #[test]
    fn parse_env_line_blocks_dyld_insert_libraries() {
        let result = parse_env_line("DYLD_INSERT_LIBRARIES=/evil/lib.dylib");
        assert!(result.is_err(), "expected error for DYLD_INSERT_LIBRARIES");
    }

    /// Normal env lines must pass through unchanged.
    #[test]
    fn parse_env_line_accepts_normal_keys() {
        let result = parse_env_line("MY_TOKEN=abc123").expect("parse_env_line should succeed");
        assert_eq!(result, Some(("MY_TOKEN".to_string(), "abc123".to_string())));
    }

    /// Blank lines return None.
    #[test]
    fn parse_env_line_ignores_blank() {
        assert_eq!(
            parse_env_line("").expect("parse_env_line should succeed"),
            None
        );
        assert_eq!(
            parse_env_line("   ").expect("parse_env_line should succeed"),
            None
        );
    }

    /// Comment lines return None.
    #[test]
    fn parse_env_line_ignores_comments() {
        assert_eq!(
            parse_env_line("# THIS_IS_A_COMMENT=value").expect("parse_env_line should succeed"),
            None
        );
    }

    /// Double-quoted values are unquoted.
    #[test]
    fn parse_env_line_strips_double_quotes() {
        let result = parse_env_line(r#"API_KEY="secret""#).expect("parse_env_line should succeed");
        assert_eq!(result, Some(("API_KEY".to_string(), "secret".to_string())));
    }

    /// Single-quoted values are unquoted (no escape processing).
    #[test]
    fn parse_env_line_strips_single_quotes() {
        let result = parse_env_line("MY_VAR='hello world'").expect("parse_env_line should succeed");
        assert_eq!(
            result,
            Some(("MY_VAR".to_string(), "hello world".to_string()))
        );
    }

    /// Backslash-escaped double-quote within double-quoted value is handled.
    #[test]
    fn parse_env_line_unescapes_inner_double_quote() {
        let result = parse_env_line(r#"MSG="val\"ue""#).expect("parse_env_line should succeed");
        assert_eq!(result, Some(("MSG".to_string(), r#"val"ue"#.to_string())));
    }

    /// Unquoted values pass through unchanged.
    #[test]
    fn parse_env_line_unquoted_passthrough() {
        let result = parse_env_line("TOKEN=abc123").expect("parse_env_line should succeed");
        assert_eq!(result, Some(("TOKEN".to_string(), "abc123".to_string())));
    }
}

#[cfg(test)]
mod clone_step_tests {
    use super::*;
    use repo::FakeGit;
    use std::path::Path;

    fn opts(config: PathBuf, git: FakeGit) -> BootstrapOptions {
        BootstrapOptions {
            config,
            force: false,
            key_file: Some(PathBuf::from("/nonexistent/notstrap-test-key")),
            dotfiles: None,
            tailscale: Some(None),
            check_prereqs: None,
            reconcile_tools: None,
            git: Box::new(git),
            prompter: None,
            assume_yes: false,
            secrets_config: None,
        }
    }

    fn write_config(dir: &Path, dotfiles_dir: &Path) -> PathBuf {
        let config = dir.join("notstrap.toml");
        std::fs::write(
            &config,
            format!(
                "[bootstrap]\ndotfiles_repo = \"https://example/dots.git\"\ndotfiles_dir = \"{}\"\n",
                dotfiles_dir.display()
            ),
        )
        .expect("write config");
        config
    }

    fn clone_status(report: &Report) -> Option<&StepStatus> {
        report
            .steps
            .iter()
            .find(|s| s.name == "clone dotfiles")
            .map(|s| &s.status)
    }

    #[test]
    fn a_failing_clone_is_reported_and_stops_the_run() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = write_config(dir.path(), &dir.path().join("dots"));

        let report = run(opts(config, FakeGit::failing("network down"))).expect("run");

        assert!(
            matches!(clone_status(&report), Some(StepStatus::Failed(m)) if m.contains("network down"))
        );
        assert!(report.steps.iter().all(|s| s.name != "age key"));
    }

    #[test]
    fn an_existing_checkout_skips_the_clone() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dots = dir.path().join("dots");
        std::fs::create_dir(&dots).expect("mkdir");
        let config = write_config(dir.path(), &dots);

        // The age-key step after the clone cannot succeed here; only the clone step matters.
        let report = run(opts(config, FakeGit::failing("must not be called"))).expect("run");

        assert!(matches!(clone_status(&report), Some(StepStatus::Skipped)));
    }
}

#[cfg(test)]
mod secret_env_tests {
    use super::*;
    use std::collections::HashMap;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn valid_secrets_are_returned_sorted_by_key() {
        let vars = validated_env(map(&[("ZED", "1"), ("ALPHA", "2"), ("MID", "3")]))
            .expect("all keys are safe");

        let keys: Vec<&str> = vars.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["ALPHA", "MID", "ZED"]);
    }

    #[test]
    fn quoted_values_are_unquoted() {
        let vars = validated_env(map(&[("TOKEN", "\"abc\"")])).expect("safe");
        assert_eq!(vars, [("TOKEN".to_string(), "abc".to_string())]);
    }

    #[test]
    fn a_blocked_key_fails_the_whole_set() {
        let err = validated_env(map(&[("OK", "1"), ("LD_PRELOAD", "/evil.so")]))
            .expect_err("LD_PRELOAD is blocked");
        assert!(err.to_string().contains("LD_PRELOAD"));
    }

    #[test]
    fn a_null_byte_fails_the_whole_set() {
        let err = validated_env(map(&[("KEY", "va\0lue")])).expect_err("null byte");
        assert!(err.to_string().contains("null byte"));
    }

    #[test]
    fn an_empty_set_is_fine() {
        assert!(validated_env(HashMap::new()).expect("empty").is_empty());
    }
}
