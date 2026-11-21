use std::collections::BTreeSet;

use crate::declared::strip_quotes;

const BUILTINS: &[&str] = &[
    "cd", "echo", "source", "exit", "which", "type", "history", "exec", "eval", "set", "export",
    "ls", "open",
];

/// One alias found in a shell config file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Alias {
    pub name: String,
    /// First word of the alias body, with a nushell `^` prefix and quotes removed.
    pub target: String,
    pub source: String,
    /// The alias sits inside an `if command -v <target>; then` guard, so a missing
    /// target is intentional.
    pub guarded: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Scan {
    pub aliases: Vec<Alias>,
    /// Shell functions and nu commands defined in the same text.
    pub functions: BTreeSet<String>,
}

/// Parse one line of nu (`alias a = b`), zsh/bash (`alias a='b'`) or fish (`alias a b`) syntax.
pub fn parse_alias(line: &str, source: &str) -> Option<Alias> {
    let rest = line.trim().strip_prefix("alias ")?.trim_start();
    if rest.starts_with('-') {
        return None;
    }
    let split = rest.find(|c: char| c == '=' || c.is_whitespace())?;
    let name = rest[..split].to_string();
    let value = rest[split..]
        .trim_start()
        .trim_start_matches('=')
        .trim_start();
    let value = strip_quotes(value.split('#').next().unwrap_or(value));
    let target = value.split_whitespace().next()?.trim_start_matches('^');
    Some(Alias {
        name,
        target: strip_quotes(target).to_string(),
        source: source.to_string(),
        guarded: false,
    })
}

/// Whether the target is something `PATH` lookup can meaningfully check.
pub fn is_checkable(target: &str) -> bool {
    !(target.is_empty()
        || target.starts_with("__")
        || target.starts_with('$')
        || target.starts_with('(')
        || target.starts_with('{')
        || BUILTINS.contains(&target))
}

/// The command named by `if command -v X; then`, `which X` or `$+commands[X]`.
fn guard_name(line: &str) -> Option<String> {
    let l = line.trim();
    let is_if = l.starts_with("if ") || l.starts_with("elif ");
    if !is_if || !(l.ends_with("then") || l.contains("; then")) {
        return None;
    }
    let word_after = |marker: &str| -> Option<String> {
        let tail = l.split_once(marker)?.1.trim_start();
        let w = tail
            .split(|c: char| c.is_whitespace() || matches!(c, ';' | ')' | ']'))
            .next()?;
        Some(strip_quotes(w).to_string())
    };
    word_after("command -v ")
        .or_else(|| word_after("which "))
        .or_else(|| word_after("commands["))
}

fn function_name(line: &str) -> Option<String> {
    let l = line.trim();
    let name = match l
        .strip_prefix("export def ")
        .or_else(|| l.strip_prefix("def "))
        .or_else(|| l.strip_prefix("function "))
    {
        Some(r) => r
            .trim_start_matches(['-', ' '])
            .split(|c: char| c.is_whitespace() || c == '(' || c == '[')
            .next(),
        None => l.split_once("()").map(|(n, _)| n.trim()),
    }?;
    let name = name.trim_matches('"');
    (!name.is_empty() && !name.contains(' ')).then(|| name.to_string())
}

/// Scan the text of one shell config file for aliases and function definitions.
pub fn scan(text: &str, source: &str) -> Scan {
    let mut out = Scan::default();
    let mut guards: Vec<Option<String>> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(g) = guard_name(t) {
            guards.push(Some(g));
        } else if (t.starts_with("if ") || t.starts_with("case ")) && !t.ends_with('{') {
            guards.push(None);
        } else if t == "fi" || t == "esac" {
            guards.pop();
        }
        if let Some(mut a) = parse_alias(t, source) {
            a.guarded = guards.iter().flatten().any(|g| *g == a.target);
            out.aliases.push(a);
        }
        out.functions.extend(function_name(t));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(line: &str) -> Option<String> {
        parse_alias(line, "x").map(|a| format!("{}={}", a.name, a.target))
    }

    #[test]
    fn parses_all_shell_syntaxes() {
        assert_eq!(
            p("alias clear = pyroclear").as_deref(),
            Some("clear=pyroclear")
        );
        assert_eq!(p("  alias gs='git status -sb'").as_deref(), Some("gs=git"));
        assert_eq!(
            p("alias clear pyroclear").as_deref(),
            Some("clear=pyroclear")
        );
        assert_eq!(p("alias z  = __zoxide_z").as_deref(), Some("z=__zoxide_z"));
        assert_eq!(p("alias ll = ^eza -l").as_deref(), Some("ll=eza"));
    }

    #[test]
    fn ignores_comments_and_flags() {
        assert_eq!(p("# alias a=b"), None);
        assert_eq!(p("alias -g X=y"), None);
    }

    #[test]
    fn uncheckable_targets_are_skipped() {
        assert!(!is_checkable("__zoxide_z"));
        assert!(!is_checkable("cd"));
        assert!(is_checkable("pyroclear"));
    }

    #[test]
    fn guarded_aliases_are_marked() {
        let text = "if command -v zb >/dev/null 2>&1; then\n  alias zbi='zb install'\nfi\nalias zbs='zb search'\n";
        let s = scan(text, "f");
        assert!(s.aliases.iter().find(|a| a.name == "zbi").unwrap().guarded);
        assert!(!s.aliases.iter().find(|a| a.name == "zbs").unwrap().guarded);
    }

    #[test]
    fn guard_must_match_the_target() {
        let text = "if command -v other; then\n  alias a='zb x'\nfi\n";
        assert!(!scan(text, "f").aliases[0].guarded);
    }

    #[test]
    fn nu_if_blocks_do_not_leak_guards() {
        let text = "if (true) {\n  1\n}\nalias a = zb x\n";
        assert!(!scan(text, "f").aliases[0].guarded);
    }

    #[test]
    fn collects_functions() {
        let text = "export def pj [] {}\nfunction gl() {\n}\nfoo() { :; }\n";
        let s = scan(text, "f");
        for n in ["pj", "gl", "foo"] {
            assert!(s.functions.contains(n), "missing {n}");
        }
    }
}
