use serde_json::json;

use notcore::reporter::{LinkEvent, Reporter};

/// Terminal reporter with ANSI color output.
pub struct TerminalReporter;

/// JSON reporter: one JSON object per line to stdout.
pub struct JsonReporter;

impl Reporter for TerminalReporter {
    fn report(&self, event: &LinkEvent<'_>) {
        match event {
            LinkEvent::Link { source, target } => {
                println!("  \x1b[32mlink\x1b[0m {source} -> {}", target.display());
            }
            LinkEvent::Copy { source, target } => {
                println!("  \x1b[32mcopy\x1b[0m {source} -> {}", target.display());
            }
            LinkEvent::Skip { source, reason } => {
                println!("  \x1b[90mskip\x1b[0m {source} ({reason})");
            }
            LinkEvent::Backup { from, to } => {
                println!(
                    "  \x1b[33mbackup\x1b[0m {} -> {}",
                    from.display(),
                    to.display()
                );
            }
            LinkEvent::Remove { target } => {
                println!("  \x1b[31mremove\x1b[0m {}", target.display());
            }
            LinkEvent::CreateDir { path } => {
                println!("  \x1b[90mcreate dir\x1b[0m {}", path.display());
            }
            LinkEvent::DryRun {
                action,
                source,
                target,
            } => {
                println!(
                    "  \x1b[36mwould {action}\x1b[0m {source} -> {}",
                    target.display()
                );
            }
        }
    }
}

impl Reporter for JsonReporter {
    fn report(&self, event: &LinkEvent<'_>) {
        let obj = match event {
            LinkEvent::Link { source, target } => {
                json!({"event": "link", "source": source, "target": target.to_string_lossy()})
            }
            LinkEvent::Copy { source, target } => {
                json!({"event": "copy", "source": source, "target": target.to_string_lossy()})
            }
            LinkEvent::Skip { source, reason } => {
                json!({"event": "skip", "source": source, "reason": reason})
            }
            LinkEvent::Backup { from, to } => {
                json!({"event": "backup", "from": from.to_string_lossy(), "to": to.to_string_lossy()})
            }
            LinkEvent::Remove { target } => {
                json!({"event": "remove", "target": target.to_string_lossy()})
            }
            LinkEvent::CreateDir { path } => {
                json!({"event": "create_dir", "path": path.to_string_lossy()})
            }
            LinkEvent::DryRun {
                action,
                source,
                target,
            } => {
                json!({"event": "dry_run", "action": action, "source": source, "target": target.to_string_lossy()})
            }
        };
        println!("{}", serde_json::to_string(&obj).unwrap_or_default());
    }
}
