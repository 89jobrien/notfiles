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

// TODO(install): `notfiles` is not yet installed on PATH. Add `just install` recipe and
//   `mise task` so `nf` / `notfiles` is available without `cargo run`. See Justfile.

// TODO(bootstrap): Wire `notstrap` into the CLI as `notfiles bootstrap`. Should clone the
//   repo, resolve age key (Bitwarden/YubiKey/prompt), decrypt SOPS secrets, link packages,
//   and run post-hooks. The notstrap crate already models this — needs a CLI entry point.

// TODO(migrate): Add `notfiles migrate <dir>` that takes a stow/chezmoi repo (from `detect`)
//   and generates a notfiles.toml for it, then bulk-adopts all packages. Should call
//   `detect::detect()`, let the user pick a source, and run `adopt` for each package.

// TODO(which): Add `notfiles which <path>` — reverse lookup from a target path (e.g.
//   ~/.gitconfig) to its source package and file. Walk state entries and check read_link.

// TODO(doctor): Add `notfiles doctor` — holistic drift detection replacing drift-check.sh.
//   Checks: broken symlinks, unmanaged dotfiles in $HOME, dirty git state, conflicts,
//   packages in dir not listed in notfiles.toml. Should print a summary like `just doctor`.

// TODO(discover): Add `[defaults] discover = true` mode to notfiles.toml so all top-level
//   dirs are treated as packages automatically, without maintaining an explicit `include`
//   list. Opt-in via config, not the default (explicit is safer).

// TODO(edit): Add `notfiles edit <managed-path>` — resolve the source file for a managed
//   symlink and open it in $EDITOR. Quality-of-life shortcut for day-to-day editing.

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
