use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "notfiles", about = "A modern dotfiles manager")]
pub struct Cli {
    /// Path to the dotfiles directory (default: current directory)
    #[arg(long, global = true)]
    pub dir: Option<PathBuf>,

    /// Show what would be done without making changes
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Show verbose output
    #[arg(long, short, global = true)]
    pub verbose: bool,

    /// Output in JSON format
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a starter notfiles.toml
    Init,

    /// Create symlinks/copies for packages
    Link {
        /// Overwrite existing files (backs up first)
        #[arg(long)]
        force: bool,

        /// Skip creating backups when using --force
        #[arg(long)]
        no_backup: bool,

        /// Specific packages to link (default: all)
        packages: Vec<String>,
    },

    /// Remove managed symlinks/copies
    Unlink {
        /// Specific packages to unlink (default: all)
        packages: Vec<String>,
    },

    /// Show link state per package
    Status {
        /// Specific packages to check (default: all)
        packages: Vec<String>,
    },

    /// Validate config without making changes (CI-friendly)
    Check,

    /// Generate shell completions
    Completions {
        /// Shell to generate for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Show differences between source and target for copy-method packages
    Diff {
        /// Specific packages to diff (default: all copy-method packages)
        packages: Vec<String>,
    },

    /// Auto-detect existing dotfile managers (stow, chezmoi, etc.)
    Detect,

    /// Move existing files into a package and replace with symlinks
    Adopt {
        /// Package to adopt files into
        package: String,
        /// File paths (relative to target dir) to adopt
        files: Vec<String>,
    },
}
