use std::collections::BTreeSet;

/// Strip one layer of matching quotes.
pub fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    for q in ['\'', '"'] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return &s[1..s.len() - 1];
        }
    }
    s
}

/// Normalize a mise tool key to a bare tool name: `cargo:nu` -> `nu`,
/// `aqua:rustsec/rustsec/cargo-audit` -> `cargo-audit`,
/// `cargo:https://h/o/ruff.git` -> `ruff`, `just` -> `just`.
pub fn tool_name(key: &str) -> String {
    let rest = key.split_once(':').map_or(key, |(_, r)| r);
    let last = rest.rsplit('/').next().unwrap_or(rest);
    last.trim_end_matches(".git").to_string()
}

/// Tool names declared under `[tools]` in a mise config, across all backends,
/// with quoted or bare keys.
pub fn mise_tools(mise_text: &str) -> BTreeSet<String> {
    let mut in_tools = false;
    let mut out = BTreeSet::new();
    for line in mise_text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_tools = line == "[tools]";
            continue;
        }
        if !in_tools || line.starts_with('#') {
            continue;
        }
        if let Some((key, _)) = line.split_once('=') {
            out.insert(tool_name(strip_quotes(key)));
        }
    }
    out
}

/// Formula and cask names declared in a Brewfile: `brew "tap/name"` -> `name`.
pub fn brew_names(brewfile_text: &str) -> BTreeSet<String> {
    brewfile_text
        .lines()
        .map(str::trim)
        .filter_map(|l| l.strip_prefix("brew ").or_else(|| l.strip_prefix("cask ")))
        .filter_map(|l| l.split('"').nth(1))
        .map(|n| n.rsplit('/').next().unwrap_or(n).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_mise_tools_across_backends() {
        let m = "[env]\nFOO = \"x\"\n[tools]\n\"cargo:nu\" = \"1\"\n\"aqua:rustsec/rustsec/cargo-audit\" = \"2\"\njust = \"1.40.0\"\n\"cargo:https://git.example.com/o/ruff.git\" = \"tag:1\"\n# \"cargo:skipped\" = \"1\"\n[tasks.x]\nrun = \"y\"\n";
        let t = mise_tools(m);
        for want in ["nu", "cargo-audit", "just", "ruff"] {
            assert!(t.contains(want), "missing {want}");
        }
        for not in ["FOO", "run", "skipped"] {
            assert!(!t.contains(not), "unexpected {not}");
        }
    }

    #[test]
    fn reads_brew_names() {
        let b = "tap \"x/y\"\nbrew \"stow\"   # c\nbrew \"homebrew/core/bacon\"\ncask \"zed\"\n";
        let n = brew_names(b);
        assert!(n.contains("stow") && n.contains("bacon") && n.contains("zed"));
        assert!(!n.contains("x/y"));
    }
}
