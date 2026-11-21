use anyhow::Result;
use which::which;

/// Commands required before the manifest-driven steps can run, used when `notstrap.toml`
/// has no `[prereqs]` table.
pub const DEFAULT_REQUIRED: &[&str] = &["nu", "sops", "age"];

pub fn default_required() -> Vec<String> {
    DEFAULT_REQUIRED.iter().map(|s| s.to_string()).collect()
}

/// How to install a missing command. Package names match the command name for every
/// tool notstrap requires.
pub fn install_hint(cmd: &str) -> String {
    format!("brew install {cmd}  OR  nix-env -iA nixpkgs.{cmd}")
}

/// The subset of `required` for which `found` is false, in order.
pub fn missing(required: &[String], found: impl Fn(&str) -> bool) -> Vec<&str> {
    required
        .iter()
        .map(String::as_str)
        .filter(|c| !found(c))
        .collect()
}

pub fn check_prerequisites(required: &[String]) -> Result<()> {
    let missing = missing(required, |c| which(c).is_ok());
    if missing.is_empty() {
        return Ok(());
    }
    eprintln!("Missing required tools:\n");
    for c in &missing {
        eprintln!("  {c} — {}", install_hint(c));
    }
    anyhow::bail!("{} prerequisite(s) missing", missing.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_historical_list() {
        assert_eq!(default_required(), ["nu", "sops", "age"]);
    }

    #[test]
    fn reports_only_missing_commands_in_order() {
        let req = vec!["nu".to_string(), "sops".to_string(), "age".to_string()];
        assert_eq!(missing(&req, |c| c == "nu"), ["sops", "age"]);
        assert!(missing(&req, |_| true).is_empty());
    }

    #[test]
    fn hint_names_the_command() {
        assert!(install_hint("age").contains("brew install age"));
    }
}
