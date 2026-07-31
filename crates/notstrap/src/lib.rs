use anyhow::{Context, Result};
use notcore::{HookPhase, Report, StepStatus};
use notfiles::{LinkOptions, link};
use nothooks::{HookRunner, run_phase};
use notsecrets::sources::IdentitySource;
use notsecrets::{
    BitwardenSource, FileSource, PromptSource, SecretResolver, YubikeySource, resolve_identities,
};
use serde::Deserialize;
use std::path::PathBuf;

pub mod forge;
pub mod prereqs;
pub mod repo;

pub use notnet::TailscaleOptions;

#[derive(Deserialize)]
pub struct NotstrapConfig {
    pub bootstrap: BootstrapSection,
    /// When present, notstrap joins the tailnet before cloning dotfiles.
    pub tailscale: Option<TailscaleOptions>,
    #[serde(default)]
    pub hooks: Vec<notcore::HookSpec>,
    /// When present and enabled, notstrap ensures the configured forge
    /// repo exists and is wired as a git remote before cloning dotfiles.
    pub forge: Option<ForgeSection>,
}

/// Optional `[forge]` bootstrap step configuration.
///
/// Deliberately primitive-shaped (paths and flags) rather than embedding
/// `notforge::ForgeConfig` directly, so `notstrap.toml` stays insulated
/// from `notforge`'s config schema, which is not yet stable.
#[derive(Deserialize)]
pub struct ForgeSection {
    #[serde(default)]
    pub enabled: bool,
    /// Path to a notforge `ForgeConfig` TOML file.
    pub config_path: PathBuf,
    /// Path to the local repo notstrap should wire a remote onto.
    pub local_repo: PathBuf,
    #[serde(default)]
    pub on_error: ForgeOnError,
}

/// How a failed forge step affects the rest of the bootstrap run.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ForgeOnError {
    /// Log the failure and continue bootstrapping.
    #[default]
    Warn,
    /// Abort bootstrap when the forge step fails.
    Fail,
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
    /// None = skip prereq check (tests). Some(f) = run f().
    pub check_prereqs: Option<Box<dyn Fn() -> Result<()>>>,
    /// Path to notsecrets.toml. None = skip secret resolution.
    pub secrets_config: Option<PathBuf>,
}

pub fn run(opts: BootstrapOptions) -> Result<Report> {
    let mut report = Report::default();

    // 1. Prerequisites
    if let Some(check) = opts.check_prereqs {
        match check() {
            Ok(_) => {
                report.add("prerequisites", StepStatus::Ok);
            }
            Err(e) => {
                report.add("prerequisites", StepStatus::Failed(e.to_string()));
                return Ok(report);
            }
        }
    }

    // 2. Load config
    let config_content = std::fs::read_to_string(&opts.config)
        .with_context(|| format!("cannot read {}", opts.config.display()))?;
    let cfg: NotstrapConfig = toml::from_str(&config_content)?;

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
    match repo::clone_if_missing(&clone_url, &dotfiles_dir) {
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

    // 5b. Resolve secrets via notsecrets.toml (optional)
    if let Some(secrets_path) = opts.secrets_config {
        match notsecrets::load_config(&secrets_path) {
            Ok(secrets_cfg) => match SecretResolver::from_config(secrets_cfg) {
                Ok(resolver) => match resolver.resolve_all() {
                    Ok(env_map) => {
                        for (k, v) in &env_map {
                            match parse_env_line(&format!("{k}={v}")) {
                                Ok(Some((k, v))) => {
                                    // SAFETY: `notstrap` is a single-threaded bootstrap
                                    // binary; no other threads exist. Keys are validated
                                    // by parse_env_line (null bytes and dangerous keys
                                    // are rejected).
                                    unsafe {
                                        std::env::set_var(&k, &v);
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    report.add("secrets", StepStatus::Failed(e.to_string()));
                                    return Ok(report);
                                }
                            }
                        }
                        report.add("secrets", StepStatus::Ok);
                    }
                    Err(e) => {
                        report.add("secrets", StepStatus::Failed(e.to_string()));
                        return Ok(report);
                    }
                },
                Err(e) => {
                    report.add("secrets", StepStatus::Failed(e.to_string()));
                    return Ok(report);
                }
            },
            Err(e) => {
                report.add("secrets", StepStatus::Failed(e.to_string()));
                return Ok(report);
            }
        }
    }

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
    let runner = if opts.force {
        HookRunner::with_force(dotfiles_dir.clone())
    } else {
        HookRunner::new(dotfiles_dir.clone())
    };

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

    Ok(report)
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

    fn minimal_toml() -> &'static str {
        r#"
        [bootstrap]
        dotfiles_dir = "~/.notfiles"
        dotfiles_repo = "https://example.com/dotfiles.git"
        "#
    }

    #[test]
    fn forge_section_absent_by_default() {
        let cfg: NotstrapConfig = toml::from_str(minimal_toml()).unwrap();
        assert!(cfg.forge.is_none());
    }

    #[test]
    fn forge_section_parses_when_present() {
        let toml_str = r#"
        [bootstrap]
        dotfiles_dir = "~/.notfiles"
        dotfiles_repo = "https://example.com/dotfiles.git"

        [forge]
        enabled = true
        config_path = "forge.toml"
        local_repo = "~/.notfiles"
        on_error = "warn"
        "#;
        let cfg: NotstrapConfig = toml::from_str(toml_str).unwrap();
        let forge = cfg.forge.unwrap();
        assert!(forge.enabled);
        assert_eq!(forge.on_error, ForgeOnError::Warn);
    }

    #[test]
    fn forge_on_error_defaults_to_warn() {
        let toml_str = r#"
        [bootstrap]
        dotfiles_dir = "~/.notfiles"
        dotfiles_repo = "https://example.com/dotfiles.git"

        [forge]
        enabled = true
        config_path = "forge.toml"
        local_repo = "~/.notfiles"
        "#;
        let cfg: NotstrapConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.forge.unwrap().on_error, ForgeOnError::Warn);
    }

    #[test]
    fn forge_enabled_defaults_to_false() {
        let toml_str = r#"
        [bootstrap]
        dotfiles_dir = "~/.notfiles"
        dotfiles_repo = "https://example.com/dotfiles.git"

        [forge]
        config_path = "forge.toml"
        local_repo = "~/.notfiles"
        "#;
        let cfg: NotstrapConfig = toml::from_str(toml_str).unwrap();
        assert!(!cfg.forge.unwrap().enabled);
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
        let result = parse_env_line("MY_TOKEN=abc123").unwrap();
        assert_eq!(result, Some(("MY_TOKEN".to_string(), "abc123".to_string())));
    }

    /// Blank lines return None.
    #[test]
    fn parse_env_line_ignores_blank() {
        assert_eq!(parse_env_line("").unwrap(), None);
        assert_eq!(parse_env_line("   ").unwrap(), None);
    }

    /// Comment lines return None.
    #[test]
    fn parse_env_line_ignores_comments() {
        assert_eq!(parse_env_line("# THIS_IS_A_COMMENT=value").unwrap(), None);
    }

    /// Double-quoted values are unquoted.
    #[test]
    fn parse_env_line_strips_double_quotes() {
        let result = parse_env_line(r#"API_KEY="secret""#).unwrap();
        assert_eq!(result, Some(("API_KEY".to_string(), "secret".to_string())));
    }

    /// Single-quoted values are unquoted (no escape processing).
    #[test]
    fn parse_env_line_strips_single_quotes() {
        let result = parse_env_line("MY_VAR='hello world'").unwrap();
        assert_eq!(
            result,
            Some(("MY_VAR".to_string(), "hello world".to_string()))
        );
    }

    /// Backslash-escaped double-quote within double-quoted value is handled.
    #[test]
    fn parse_env_line_unescapes_inner_double_quote() {
        let result = parse_env_line(r#"MSG="val\"ue""#).unwrap();
        assert_eq!(result, Some(("MSG".to_string(), r#"val"ue"#.to_string())));
    }

    /// Unquoted values pass through unchanged.
    #[test]
    fn parse_env_line_unquoted_passthrough() {
        let result = parse_env_line("TOKEN=abc123").unwrap();
        assert_eq!(result, Some(("TOKEN".to_string(), "abc123".to_string())));
    }
}
