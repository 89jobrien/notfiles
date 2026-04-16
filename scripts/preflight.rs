#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! colored = "2"
//! ```
//!
//! Quick preflight context surfacer:
//!   - current shell detection
//!   - git status + recent history
//!   - tracked-file tree (git ls-files)
//!   - HANDOFF.* document summary
//!
//! Usage: ./scripts/preflight.rs [dir]

use anyhow::{Context, Result};
use colored::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run(cmd: &str, args: &[&str], cwd: &Path) -> Result<String> {
    let out = Command::new(cmd)
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("failed to run `{cmd}`"))?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn run_opt(cmd: &str, args: &[&str], cwd: &Path) -> String {
    run(cmd, args, cwd).unwrap_or_default()
}

// ── Shell detection ───────────────────────────────────────────────────────────

fn detect_shell() -> String {
    // SHELL env is the login shell; $0 in a running session is more precise but
    // not accessible from a child process, so we use SHELL + FISH_VERSION hint.
    let shell_env = std::env::var("SHELL").unwrap_or_else(|_| "unknown".into());
    let name = Path::new(&shell_env)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Nushell doesn't set SHELL; detect via NU_VERSION if present.
    if std::env::var("NU_VERSION").is_ok() || name == "nu" {
        return "nushell".into();
    }
    if std::env::var("FISH_VERSION").is_ok() {
        return "fish".into();
    }
    name
}

// ── Git helpers ───────────────────────────────────────────────────────────────

fn git_root(cwd: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        Some(PathBuf::from(s))
    } else {
        None
    }
}

fn git_branch(root: &Path) -> String {
    run_opt("git", &["branch", "--show-current"], root)
        .trim()
        .to_string()
}

fn git_status(root: &Path) -> String {
    run_opt("git", &["status", "--short"], root)
}

fn git_log(root: &Path) -> String {
    run_opt(
        "git",
        &["log", "--oneline", "-7", "--decorate=short"],
        root,
    )
}

// ── Tracked-file tree ─────────────────────────────────────────────────────────

/// Build a tree from `git ls-files` output and render it as a compact indented
/// string. Each directory node is printed once; files are leaves.
fn tracked_tree(root: &Path) -> String {
    let raw = run_opt("git", &["ls-files"], root);
    let paths: Vec<&str> = raw.lines().collect();

    // Group by first path component for a two-level summary.
    let mut dirs: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut top_files: Vec<&str> = Vec::new();

    for p in &paths {
        if let Some(slash) = p.find('/') {
            dirs.entry(&p[..slash]).or_default().push(&p[slash + 1..]);
        } else {
            top_files.push(p);
        }
    }

    let mut out = String::new();
    for f in &top_files {
        out.push_str(&format!("  {f}\n"));
    }
    for (dir, files) in &dirs {
        out.push_str(&format!("  {dir}/  ({} files)\n", files.len()));
        // show up to 6 entries per dir to keep output compact
        for f in files.iter().take(6) {
            out.push_str(&format!("    {f}\n"));
        }
        if files.len() > 6 {
            out.push_str(&format!("    … and {} more\n", files.len() - 6));
        }
    }
    out
}

// ── HANDOFF surfacer ──────────────────────────────────────────────────────────

fn surface_handoffs(root: &Path) -> Vec<(String, String)> {
    let mut results = Vec::new();

    // Collect HANDOFF.* from root and .ctx/
    let candidates: &[&str] = &["HANDOFF.md", ".ctx"];
    for c in candidates {
        let p = root.join(c);
        if p.is_dir() {
            // Read all HANDOFF yaml files in .ctx/
            if let Ok(rd) = std::fs::read_dir(&p) {
                let mut entries: Vec<_> = rd.flatten().collect();
                entries.sort_by_key(|e| e.file_name());
                for entry in entries {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("HANDOFF") && !name.ends_with(".state.yaml") {
                        let content =
                            std::fs::read_to_string(entry.path()).unwrap_or_default();
                        results.push((format!(".ctx/{name}"), summarize_handoff(&content)));
                    }
                }
            }
        } else if p.is_file() {
            let content = std::fs::read_to_string(&p).unwrap_or_default();
            results.push((c.to_string(), summarize_handoff(&content)));
        }
    }
    results
}

/// Extract the most useful lines from a HANDOFF file (yaml or md).
fn summarize_handoff(content: &str) -> String {
    // For YAML: grab project, branch, items, last log entry.
    // For MD: grab the first 20 non-blank lines.
    let lines: Vec<&str> = content.lines().collect();

    if content.starts_with("project:") || content.contains("items:") {
        // YAML path — grab key fields
        let mut summary = Vec::new();
        let mut in_items = false;
        let mut item_count = 0;
        let mut last_log = String::new();
        let mut in_log = false;

        for line in &lines {
            let t = line.trim();
            if t.starts_with("project:") || t.starts_with("branch:") || t.starts_with("updated:") {
                summary.push(t.to_string());
            }
            if t == "items: []" {
                summary.push("items: (none)".into());
            }
            if t.starts_with("- id:") || t.starts_with("- title:") {
                in_items = true;
                item_count += 1;
                summary.push(format!("  item: {}", t.trim_start_matches("- ")));
            }
            if t == "log:" {
                in_log = true;
            }
            if in_log && t.starts_with("summary:") {
                last_log = t.trim_start_matches("summary:").trim().to_string();
            }
        }
        if item_count > 0 {
            summary.push(format!("total items: {item_count}"));
        }
        if !last_log.is_empty() {
            summary.push(format!("last log: {last_log}"));
        }
        summary.join("\n")
    } else {
        // MD path — first 20 non-blank lines
        lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .take(20)
            .copied()
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn section(title: &str) {
    println!("\n{}", format!("── {title} ").bold().cyan());
}

fn main() -> Result<()> {
    let cwd = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    println!("{}", "╔══════════════════════════════╗".bold());
    println!("{}", "║  preflight context surfacer  ║".bold());
    println!("{}", "╚══════════════════════════════╝".bold());

    // ── Shell ────────────────────────────────────────────────────────────────
    section("shell");
    println!("  {}", detect_shell().green());

    // ── Git root ─────────────────────────────────────────────────────────────
    section("workspace");
    let root = git_root(&cwd);
    match &root {
        Some(r) => {
            let branch = git_branch(r);
            println!("  root:   {}", r.display().to_string().yellow());
            println!("  branch: {}", branch.green());
        }
        None => {
            println!("  {}", "not a git repository".red());
        }
    }

    if let Some(root) = &root {
        // ── Git status ───────────────────────────────────────────────────────
        section("git status");
        let status = git_status(root);
        if status.trim().is_empty() {
            println!("  {}", "clean".green());
        } else {
            for line in status.lines() {
                // color by status prefix
                let colored = if line.starts_with('M') || line.starts_with("RM") {
                    line.yellow().to_string()
                } else if line.starts_with('A') {
                    line.green().to_string()
                } else if line.starts_with('D') || line.starts_with('R') {
                    line.red().to_string()
                } else if line.starts_with('?') {
                    line.dimmed().to_string()
                } else {
                    line.normal().to_string()
                };
                println!("  {colored}");
            }
        }

        // ── Git log ──────────────────────────────────────────────────────────
        section("recent commits");
        for line in git_log(root).lines() {
            println!("  {line}");
        }

        // ── File tree ────────────────────────────────────────────────────────
        section("tracked files");
        print!("{}", tracked_tree(root));

        // ── HANDOFF ──────────────────────────────────────────────────────────
        section("handoff");
        let handoffs = surface_handoffs(root);
        if handoffs.is_empty() {
            println!("  {}", "no HANDOFF files found".dimmed());
        } else {
            for (name, summary) in &handoffs {
                println!("  {}", name.bold().yellow());
                for line in summary.lines() {
                    println!("    {line}");
                }
                println!();
            }
        }
    }

    println!("{}", "─────────────────────────────────".dimmed());
    Ok(())
}
