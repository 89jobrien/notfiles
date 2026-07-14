# preflight.rs — Quick Context Surfacer

A `rust-script` preflight that surfaces repo context in one shot: shell detection, git state,
tracked-file tree, and HANDOFF document read/update.

````rust
#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! anyhow   = "1"
//! colored  = "2"
//! chrono   = { version = "0.4", features = ["clock"] }
//! ```

use anyhow::{Context, Result};
use colored::Colorize;
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn run_or(cmd: &str, args: &[&str], fallback: &str) -> String {
    run(cmd, args).unwrap_or_else(|| fallback.to_string())
}

fn section(title: &str) {
    println!("\n{}", format!("── {title} ").bold().cyan());
}

// ---------------------------------------------------------------------------
// Shell detection
// ---------------------------------------------------------------------------

fn detect_shell() -> String {
    // SHELL env is most reliable; fall back to $0 via ps
    env::var("SHELL")
        .ok()
        .and_then(|s| Path::new(&s).file_name().map(|n| n.to_string_lossy().into_owned()))
        .or_else(|| {
            // Ask the parent process name via `ps`
            let ppid = run("sh", &["-c", "echo $PPID"])?;
            run("ps", &["-p", &ppid, "-o", "comm="])
        })
        .unwrap_or_else(|| "unknown".into())
}

// ---------------------------------------------------------------------------
// Git
// ---------------------------------------------------------------------------

fn git_root() -> Option<PathBuf> {
    run("git", &["rev-parse", "--show-toplevel"]).map(PathBuf::from)
}

fn git_branch() -> String {
    run_or("git", &["branch", "--show-current"], "(detached)")
}

fn git_status_short() -> String {
    run_or("git", &["status", "--short"], "")
}

fn git_log_oneline(n: usize) -> String {
    run_or(
        "git",
        &["log", "--oneline", &format!("-{n}"), "--decorate"],
        "(no commits)",
    )
}

// ---------------------------------------------------------------------------
// Tracked-file tree (git ls-files, depth-limited)
// ---------------------------------------------------------------------------

fn tracked_tree(root: &Path, max_depth: usize) -> String {
    let files = run("git", &["ls-files"]).unwrap_or_default();
    let mut tree: Vec<Vec<String>> = Vec::new();

    for path in files.lines() {
        let parts: Vec<String> = path.split('/').map(String::from).collect();
        if parts.len() <= max_depth + 1 {
            tree.push(parts);
        }
    }

    // Deduplicate and build a simple indented representation
    let mut seen = std::collections::BTreeSet::new();
    let mut lines = Vec::new();

    for parts in &tree {
        for depth in 0..parts.len() {
            let key = parts[..=depth].join("/");
            if seen.insert(key) {
                let indent = "  ".repeat(depth);
                let is_leaf = depth == parts.len() - 1;
                let label = if is_leaf {
                    parts[depth].normal().to_string()
                } else {
                    parts[depth].bold().blue().to_string()
                };
                lines.push(format!("{indent}{label}"));
            }
        }
    }

    if lines.is_empty() {
        "(no tracked files)".to_string()
    } else {
        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// HANDOFF documents
// ---------------------------------------------------------------------------

fn find_handoff_files(root: &Path) -> Vec<PathBuf> {
    let mut hits = Vec::new();
    if let Ok(rd) = fs::read_dir(root) {
        for entry in rd.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("HANDOFF") {
                hits.push(entry.path());
            }
        }
    }
    hits.sort();
    hits
}

fn read_handoff(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))
}

fn stamp_handoff(path: &Path) -> Result<()> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %z").to_string();
    let original = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;

    // Insert or replace a `last_preflight:` line at the top of the YAML/MD doc
    let stamp_line = format!("last_preflight: {now}");
    let updated = if let Some(pos) = original.find("last_preflight:") {
        let end = original[pos..].find('\n').map(|i| pos + i + 1).unwrap_or(original.len());
        format!("{}{stamp_line}\n{}", &original[..pos], &original[end..])
    } else {
        // Prepend before first non-comment, non-empty line
        format!("{stamp_line}\n{original}")
    };

    if updated != original {
        fs::write(path, &updated)
            .with_context(|| format!("writing {}", path.display()))?;
        println!("  {} updated timestamp", path.display().to_string().dimmed());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let cwd = env::current_dir()?;
    let root = git_root().unwrap_or_else(|| cwd.clone());

    println!("{}", "╔══ preflight ══╗".bold().yellow());

    // ── Environment ──────────────────────────────────────────────────────────
    section("Environment");
    println!("  shell    : {}", detect_shell().green());
    println!("  cwd      : {}", cwd.display().to_string().green());
    println!("  git root : {}", root.display().to_string().dimmed());

    // ── Git ──────────────────────────────────────────────────────────────────
    section("Git — branch / status");
    println!("  branch   : {}", git_branch().yellow());
    let status = git_status_short();
    if status.is_empty() {
        println!("  status   : {}", "clean".green());
    } else {
        for line in status.lines() {
            println!("  {line}");
        }
    }

    section("Git — recent history");
    for line in git_log_oneline(7).lines() {
        println!("  {line}");
    }

    // ── Tracked files ─────────────────────────────────────────────────────
    section("Tracked files (depth ≤ 3)");
    println!("{}", tracked_tree(&root, 3));

    // ── HANDOFF documents ────────────────────────────────────────────────
    section("HANDOFF documents");
    let handoffs = find_handoff_files(&root);
    if handoffs.is_empty() {
        println!("  (none found)");
    } else {
        for path in &handoffs {
            println!("\n  {}", path.display().to_string().bold());
            match read_handoff(path) {
                Ok(contents) => {
                    // Print first 40 lines to avoid flooding
                    for line in contents.lines().take(40) {
                        println!("  {line}");
                    }
                    let total = contents.lines().count();
                    if total > 40 {
                        println!("  {} … ({} more lines)", "↳".dimmed(), total - 40);
                    }
                    stamp_handoff(path)?;
                }
                Err(e) => println!("  {}", format!("error: {e}").red()),
            }
        }
    }

    println!("\n{}", "╚══ done ══╝".bold().yellow());
    Ok(())
}
````

## Usage

```bash
# Make executable and run directly
chmod +x preflight.rs
./preflight.rs

# Or invoke via rust-script explicitly
rust-script preflight.rs
```

## What it surfaces

| Section           | Details                                                                                                |
| ----------------- | ------------------------------------------------------------------------------------------------------ |
| Environment       | Current shell (`$SHELL`), cwd, git root                                                                |
| Git status        | Branch name, short status (clean or dirty file list)                                                   |
| Git history       | Last 7 commits, one-line with decoration                                                               |
| Tracked file tree | `git ls-files` output rendered as an indented tree, depth ≤ 3                                          |
| HANDOFF docs      | Full content (first 40 lines) of any `HANDOFF.*` file at repo root; stamps `last_preflight:` timestamp |

## Dependencies

- `rust-script` — `cargo install rust-script` or `mise use -g rust-script`
- `anyhow`, `colored`, `chrono` — fetched automatically by rust-script on first run
