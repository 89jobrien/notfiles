# notfiles Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure the `notfiles` crate into a Cargo workspace of five focused crates (`notcore`, `notfiles`, `notsecrets`, `nothooks`, `notstrap`) that together replace the dotfiles shell-script ecosystem and provide a single-command new-machine bootstrap.

**Architecture:** `notcore` is a pure library holding shared types; `notfiles` is the existing stow engine refactored to import from `notcore`; `notsecrets` retrieves the age key via Bitwarden/file/prompt and decrypts `secrets.sops.env`; `nothooks` runs bootstrap hooks in phases; `notstrap` orchestrates all of them on a fresh machine.

**Tech Stack:** Rust 2024 edition, Cargo workspace, `clap` (derive), `serde`/`toml`, `thiserror`, `anyhow`, `globset`, `dirs`, `rpassword`, `which`, `tempfile`/`assert_fs` (tests)

> **Convention:** All hook scripts are Nushell (`.nu`). No `.sh` scripts. `nothooks` executes hooks via `nu <script>`, not `sh -c`.

---

## File Map

### New files created

```
Cargo.toml                                   # workspace root
crates/notcore/Cargo.toml
crates/notcore/src/lib.rs                    # re-exports all notcore modules
crates/notcore/src/error.rs                  # NotfilesError (moved from src/error.rs)
crates/notcore/src/config.rs                 # Config, Defaults, PackageConfig, Method (moved)
crates/notcore/src/paths.rs                  # expand_tilde, dotfiles_dir (moved)
crates/notcore/src/types.rs                  # HookSpec, PackageSpec, Report, Step, HookPhase
crates/notfiles/Cargo.toml
crates/notfiles/src/lib.rs                   # pub mod + pub fn link(), unlink(), status()
crates/notfiles/src/main.rs                  # thin CLI wrapper (moved from src/main.rs)
crates/notfiles/src/cli.rs                   # Cli, Command (moved from src/cli.rs)
crates/notfiles/src/linker.rs                # moved from src/linker.rs, imports notcore
crates/notfiles/src/package.rs               # moved from src/package.rs, imports notcore
crates/notfiles/src/ignore.rs                # moved from src/ignore.rs
crates/notfiles/src/status.rs                # moved from src/status.rs, imports notcore
crates/notfiles/tests/integration.rs         # moved from tests/integration.rs
crates/notsecrets/Cargo.toml
crates/notsecrets/src/lib.rs                 # AgeKeySource trait + resolve(), decrypt_sops()
crates/notsecrets/src/sources/mod.rs         # re-exports all sources
crates/notsecrets/src/sources/bitwarden.rs   # BitwardenSource
crates/notsecrets/src/sources/file.rs        # FileSource
crates/notsecrets/src/sources/prompt.rs      # PromptSource
crates/notsecrets/tests/integration.rs       # integration tests with temp dirs
crates/nothooks/Cargo.toml
crates/nothooks/src/lib.rs                   # pub fn run_phase(), HookRunner
crates/nothooks/src/main.rs                  # CLI wrapper
crates/nothooks/src/runner.rs                # execute_hook(), capture output
crates/nothooks/src/state.rs                 # HookState, .nothooks-state.toml persistence
crates/nothooks/tests/integration.rs
crates/notstrap/Cargo.toml
crates/notstrap/src/main.rs                  # orchestrates everything
crates/notstrap/src/prereqs.rs               # check_prerequisites()
crates/notstrap/src/repo.rs                  # clone_if_missing()
```

### Files deleted after migration

```
src/main.rs
src/cli.rs
src/config.rs
src/error.rs
src/ignore.rs
src/linker.rs
src/package.rs
src/paths.rs
src/status.rs
tests/integration.rs
Cargo.toml  (replaced by workspace root Cargo.toml)
```

---

## Task 1: Create workspace root and `notcore` crate

**Files:**
- Create: `Cargo.toml` (workspace root, replaces existing)
- Create: `crates/notcore/Cargo.toml`
- Create: `crates/notcore/src/lib.rs`
- Create: `crates/notcore/src/error.rs`
- Create: `crates/notcore/src/config.rs`
- Create: `crates/notcore/src/paths.rs`
- Create: `crates/notcore/src/types.rs`

- [ ] **Step 1: Rename existing Cargo.toml out of the way**

```bash
mv Cargo.toml Cargo.toml.old
mv Cargo.lock Cargo.lock.old
```

- [ ] **Step 2: Create workspace root `Cargo.toml`**

```toml
[workspace]
members = [
    "crates/notcore",
    "crates/notfiles",
    "crates/notsecrets",
    "crates/nothooks",
    "crates/notstrap",
]
resolver = "2"

[workspace.dependencies]
anyhow      = "1"
clap        = { version = "4", features = ["derive"] }
chrono      = { version = "0.4", default-features = false, features = ["clock"] }
dirs        = "6"
globset     = "0.4"
serde       = { version = "1", features = ["derive"] }
thiserror   = "2"
toml        = "0.8"
which       = "7"
rpassword   = "0.7"
tempfile    = "3"
assert_fs   = "1"
notcore     = { path = "crates/notcore" }
notfiles    = { path = "crates/notfiles" }
notsecrets  = { path = "crates/notsecrets" }
nothooks    = { path = "crates/nothooks" }
```

- [ ] **Step 3: Create `crates/notcore/Cargo.toml`**

```toml
[package]
name    = "notcore"
version = "0.1.0"
edition = "2024"

[dependencies]
serde     = { workspace = true }
toml      = { workspace = true }
thiserror = { workspace = true }
dirs      = { workspace = true }
anyhow    = { workspace = true }
```

- [ ] **Step 4: Create `crates/notcore/src/error.rs`**

Copy `src/error.rs` verbatim, removing the `use crate::` imports (they're now in the same crate):

```rust
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum NotfilesError {
    #[error("config file error: {0}")]
    Config(String),

    #[error("package not found: {name}")]
    PackageNotFound { name: String },

    #[error("conflict at {path}: {reason}")]
    Conflict { path: PathBuf, reason: String },

    #[error("path error: {0}")]
    Path(String),

    #[error("state file error: {0}")]
    State(String),

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}
```

- [ ] **Step 5: Create `crates/notcore/src/config.rs`**

Copy `src/config.rs` verbatim, replacing `use crate::error::NotfilesError` with `use crate::NotfilesError`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::NotfilesError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(default)]
    pub packages: HashMap<String, PackageConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defaults {
    #[serde(default = "default_target")]
    pub target: String,
    #[serde(default = "default_ignore")]
    pub ignore: Vec<String>,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            target: default_target(),
            ignore: default_ignore(),
        }
    }
}

fn default_target() -> String {
    "~".to_string()
}

fn default_ignore() -> Vec<String> {
    vec![
        ".git".to_string(),
        ".DS_Store".to_string(),
        "README.md".to_string(),
        "LICENSE".to_string(),
        "notfiles.toml".to_string(),
        ".notfiles-state.toml".to_string(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackageConfig {
    #[serde(default)]
    pub method: Option<Method>,
    pub target: Option<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    #[default]
    Symlink,
    Copy,
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Method::Symlink => write!(f, "symlink"),
            Method::Copy => write!(f, "copy"),
        }
    }
}

impl Config {
    pub fn load(dotfiles_dir: &Path) -> Result<Self, NotfilesError> {
        let config_path = dotfiles_dir.join("notfiles.toml");
        if !config_path.exists() {
            return Ok(Config::default());
        }
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| NotfilesError::Config(format!("reading {}: {e}", config_path.display())))?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| NotfilesError::Config(format!("parsing {}: {e}", config_path.display())))?;
        Ok(config)
    }

    pub fn method_for(&self, package: &str) -> Method {
        self.packages
            .get(package)
            .and_then(|p| p.method)
            .unwrap_or_default()
    }

    pub fn target_for(&self, package: &str) -> &str {
        self.packages
            .get(package)
            .and_then(|p| p.target.as_deref())
            .unwrap_or(&self.defaults.target)
    }

    pub fn ignore_patterns_for(&self, package: &str) -> Vec<&str> {
        let mut patterns: Vec<&str> = self.defaults.ignore.iter().map(|s| s.as_str()).collect();
        if let Some(pkg) = self.packages.get(package) {
            for p in &pkg.ignore {
                patterns.push(p.as_str());
            }
        }
        patterns
    }
}

pub fn starter_toml() -> &'static str {
    r#"[defaults]
target = "~"
ignore = [".git", ".DS_Store", "README.md", "LICENSE", "notfiles.toml"]

# [packages.ssh]
# method = "copy"
# ignore = ["known_hosts"]
#
# [packages.scripts]
# target = "~/bin"
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.defaults.target, "~");
        assert!(config.defaults.ignore.contains(&".git".to_string()));
        assert_eq!(config.method_for("anything"), Method::Symlink);
        assert_eq!(config.target_for("anything"), "~");
    }

    #[test]
    fn test_parse_config() {
        let toml_str = r#"
[defaults]
target = "~"
ignore = [".git"]

[packages.ssh]
method = "copy"
ignore = ["known_hosts"]

[packages.scripts]
target = "~/bin"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.method_for("ssh"), Method::Copy);
        assert_eq!(config.method_for("scripts"), Method::Symlink);
        assert_eq!(config.target_for("scripts"), "~/bin");
        assert_eq!(config.target_for("ssh"), "~");

        let ssh_ignores = config.ignore_patterns_for("ssh");
        assert!(ssh_ignores.contains(&".git"));
        assert!(ssh_ignores.contains(&"known_hosts"));
    }
}
```

- [ ] **Step 6: Create `crates/notcore/src/paths.rs`**

Copy `src/paths.rs` verbatim, replacing `use crate::error::NotfilesError` with `use crate::NotfilesError`:

```rust
use std::path::PathBuf;

use crate::NotfilesError;

pub fn expand_tilde(path: &str) -> Result<PathBuf, NotfilesError> {
    if path == "~" {
        return dirs::home_dir()
            .ok_or_else(|| NotfilesError::Path("cannot determine home directory".into()));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        let home = dirs::home_dir()
            .ok_or_else(|| NotfilesError::Path("cannot determine home directory".into()))?;
        return Ok(home.join(rest));
    }
    Ok(PathBuf::from(path))
}

pub fn dotfiles_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join("dotfiles"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde_home() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_tilde("~").unwrap(), home);
    }

    #[test]
    fn test_expand_tilde_subpath() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_tilde("~/foo/bar").unwrap(), home.join("foo/bar"));
    }

    #[test]
    fn test_expand_tilde_absolute() {
        assert_eq!(expand_tilde("/usr/bin").unwrap(), PathBuf::from("/usr/bin"));
    }
}
```

- [ ] **Step 7: Create `crates/notcore/src/types.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookPhase {
    Dot,
    Setup,
}

impl std::fmt::Display for HookPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookPhase::Dot => write!(f, "dot"),
            HookPhase::Setup => write!(f, "setup"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSpec {
    pub name: String,
    pub script: String,
    pub phase: HookPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSpec {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepStatus {
    Ok,
    Skipped,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct Step {
    pub name: String,
    pub status: StepStatus,
}

#[derive(Debug, Default)]
pub struct Report {
    pub steps: Vec<Step>,
}

impl Report {
    pub fn add(&mut self, name: impl Into<String>, status: StepStatus) {
        self.steps.push(Step { name: name.into(), status });
    }

    pub fn print(&self) {
        for step in &self.steps {
            let icon = match &step.status {
                StepStatus::Ok => "\x1b[32m✓\x1b[0m",
                StepStatus::Skipped => "\x1b[33m-\x1b[0m",
                StepStatus::Failed(_) => "\x1b[31m✗\x1b[0m",
            };
            let detail = match &step.status {
                StepStatus::Failed(msg) => format!(" ({msg})"),
                _ => String::new(),
            };
            println!("{icon} {}{detail}", step.name);
        }
    }

    pub fn has_failures(&self) -> bool {
        self.steps.iter().any(|s| matches!(s.status, StepStatus::Failed(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_has_failures() {
        let mut r = Report::default();
        r.add("step1", StepStatus::Ok);
        assert!(!r.has_failures());
        r.add("step2", StepStatus::Failed("oops".into()));
        assert!(r.has_failures());
    }

    #[test]
    fn test_hook_phase_display() {
        assert_eq!(HookPhase::Dot.to_string(), "dot");
        assert_eq!(HookPhase::Setup.to_string(), "setup");
    }
}
```

- [ ] **Step 8: Create `crates/notcore/src/lib.rs`**

```rust
pub mod config;
pub mod error;
pub mod paths;
pub mod types;

pub use config::{Config, Defaults, Method, PackageConfig};
pub use error::NotfilesError;
pub use paths::{dotfiles_dir, expand_tilde};
pub use types::{HookPhase, HookSpec, PackageSpec, Report, Step, StepStatus};
```

- [ ] **Step 9: Verify notcore builds and tests pass**

```bash
cargo test -p notcore
```

Expected: all tests pass, no warnings.

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml crates/notcore/
git commit -m "feat(notcore): add shared types, config, paths, error crate"
```

---

## Task 2: Migrate `notfiles` into workspace crate

**Files:**
- Create: `crates/notfiles/Cargo.toml`
- Create: `crates/notfiles/src/lib.rs`
- Move+adapt: `src/main.rs` → `crates/notfiles/src/main.rs`
- Move+adapt: `src/cli.rs` → `crates/notfiles/src/cli.rs`
- Move+adapt: `src/linker.rs` → `crates/notfiles/src/linker.rs`
- Move+adapt: `src/package.rs` → `crates/notfiles/src/package.rs`
- Move+adapt: `src/ignore.rs` → `crates/notfiles/src/ignore.rs`
- Move+adapt: `src/status.rs` → `crates/notfiles/src/status.rs`
- Move: `tests/integration.rs` → `crates/notfiles/tests/integration.rs`

- [ ] **Step 1: Create `crates/notfiles/Cargo.toml`**

```toml
[package]
name    = "notfiles"
version = "0.1.0"
edition = "2024"
description = "A modern dotfiles manager — pure Rust alternative to GNU Stow"

[[bin]]
name = "notfiles"
path = "src/main.rs"

[lib]
name = "notfiles"
path = "src/lib.rs"

[dependencies]
notcore  = { workspace = true }
clap     = { workspace = true }
globset  = { workspace = true }
chrono   = { workspace = true }
serde    = { workspace = true }
toml     = { workspace = true }
anyhow   = { workspace = true }

[dev-dependencies]
tempfile  = { workspace = true }
assert_fs = { workspace = true }
```

- [ ] **Step 2: Copy source files into `crates/notfiles/src/`**

```bash
mkdir -p crates/notfiles/src crates/notfiles/tests
cp src/cli.rs     crates/notfiles/src/cli.rs
cp src/linker.rs  crates/notfiles/src/linker.rs
cp src/package.rs crates/notfiles/src/package.rs
cp src/ignore.rs  crates/notfiles/src/ignore.rs
cp src/status.rs  crates/notfiles/src/status.rs
cp tests/integration.rs crates/notfiles/tests/integration.rs
```

- [ ] **Step 3: Update imports in each copied file**

In every file under `crates/notfiles/src/`, replace `use crate::error::` with `use notcore::` and `use crate::config::` with `use notcore::`, `use crate::paths::` with `use notcore::`. Concretely, the top of each file should import from `notcore` instead of `crate`:

`crates/notfiles/src/linker.rs` — change:
```rust
// remove:
use crate::config::{Config, Method};
use crate::error::NotfilesError;
use crate::paths::expand_tilde;
// add:
use notcore::{Config, Method, NotfilesError, expand_tilde};
```

`crates/notfiles/src/package.rs` — change:
```rust
// remove:
use crate::config::Config;
use crate::error::NotfilesError;
use crate::ignore::IgnoreMatcher;
// add:
use notcore::{Config, NotfilesError};
use crate::ignore::IgnoreMatcher;
```

`crates/notfiles/src/status.rs` — change:
```rust
// remove:
use crate::config::Config;
use crate::error::NotfilesError;
use crate::linker::State;
use crate::paths::expand_tilde;
// add:
use notcore::{Config, NotfilesError, expand_tilde};
use crate::linker::State;
```

- [ ] **Step 4: Create `crates/notfiles/src/lib.rs`**

```rust
pub mod cli;
pub mod ignore;
pub mod linker;
pub mod package;
pub mod status;

use anyhow::Result;
use std::path::Path;

pub use linker::{LinkOptions, State};
pub use package::resolve_packages;

pub fn link(
    dotfiles_dir: &Path,
    packages: &[String],
    opts: &LinkOptions,
) -> Result<State> {
    let config = notcore::Config::load(dotfiles_dir)?;
    let mut state = State::load(dotfiles_dir)?;
    let pkgs = resolve_packages(dotfiles_dir, packages)?;
    for pkg in &pkgs {
        linker::link_package(dotfiles_dir, &config, &mut state, pkg, opts)?;
    }
    state.save(dotfiles_dir)?;
    Ok(state)
}

pub fn unlink(dotfiles_dir: &Path, packages: &[String], opts: &LinkOptions) -> Result<()> {
    let mut state = State::load(dotfiles_dir)?;
    let pkgs = if packages.is_empty() {
        state
            .entries
            .iter()
            .map(|e| e.package.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        packages.to_vec()
    };
    for pkg in &pkgs {
        linker::unlink_package(dotfiles_dir, &mut state, pkg, opts)?;
    }
    state.save(dotfiles_dir)
}
```

- [ ] **Step 5: Create `crates/notfiles/src/main.rs`**

Copy `src/main.rs` verbatim, replacing `mod config; mod error; mod paths;` with `use notcore::{Config, expand_tilde};` and removing those three `mod` declarations. The remaining `mod` declarations (`mod cli; mod ignore; mod linker; mod package; mod status;`) move to `lib.rs` — update `main.rs` to pull from the lib:

```rust
use anyhow::{Context, Result};
use clap::Parser;
use std::fs;

use notfiles::cli::{Cli, Command};
use notfiles::linker::{LinkOptions, State};
use notfiles::package::resolve_packages;
use notfiles::status;
use notfiles::linker;
use notcore::Config;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let dotfiles_dir = cli
        .dir
        .unwrap_or_else(|| std::env::current_dir().expect("cannot determine current directory"));
    let dotfiles_dir = fs::canonicalize(&dotfiles_dir)
        .with_context(|| format!("dotfiles directory not found: {}", dotfiles_dir.display()))?;

    match cli.command {
        Command::Init => cmd_init(&dotfiles_dir)?,
        Command::Link { force, no_backup, packages } => {
            let config = Config::load(&dotfiles_dir)?;
            let mut state = State::load(&dotfiles_dir)?;
            let pkgs = resolve_packages(&dotfiles_dir, &packages)?;
            let opts = LinkOptions { force, no_backup, dry_run: cli.dry_run, verbose: cli.verbose };
            if cli.dry_run { println!("\x1b[36m(dry run)\x1b[0m"); }
            for pkg in &pkgs {
                if cli.verbose || cli.dry_run { println!("Linking {pkg}..."); }
                linker::link_package(&dotfiles_dir, &config, &mut state, pkg, &opts)?;
            }
            if !cli.dry_run {
                state.save(&dotfiles_dir)?;
                let count: usize = pkgs.iter().map(|p| state.entries_for_package(p).len()).sum();
                println!("\x1b[32mLinked {count} file{} across {} package{}.\x1b[0m",
                    if count == 1 { "" } else { "s" },
                    pkgs.len(),
                    if pkgs.len() == 1 { "" } else { "s" });
            }
        }
        Command::Unlink { packages } => {
            let config = Config::load(&dotfiles_dir)?;
            let _ = &config;
            let mut state = State::load(&dotfiles_dir)?;
            let pkgs = if packages.is_empty() {
                state.entries.iter().map(|e| e.package.clone())
                    .collect::<std::collections::HashSet<_>>().into_iter().collect::<Vec<_>>()
            } else { packages };
            let opts = LinkOptions { force: false, no_backup: false, dry_run: cli.dry_run, verbose: cli.verbose };
            if cli.dry_run { println!("\x1b[36m(dry run)\x1b[0m"); }
            for pkg in &pkgs {
                if cli.verbose || cli.dry_run { println!("Unlinking {pkg}..."); }
                linker::unlink_package(&dotfiles_dir, &mut state, pkg, &opts)?;
            }
            if !cli.dry_run {
                state.save(&dotfiles_dir)?;
                println!("\x1b[32mUnlinked {} package{}.\x1b[0m", pkgs.len(), if pkgs.len() == 1 { "" } else { "s" });
            }
        }
        Command::Status { packages } => {
            let config = Config::load(&dotfiles_dir)?;
            let state = State::load(&dotfiles_dir)?;
            let pkgs = resolve_packages(&dotfiles_dir, &packages)?;
            for pkg in &pkgs {
                let entries = status::package_status(&dotfiles_dir, &config, &state, pkg);
                status::print_status(pkg, &entries);
            }
        }
    }
    Ok(())
}

fn cmd_init(dotfiles_dir: &std::path::Path) -> Result<()> {
    let config_path = dotfiles_dir.join("notfiles.toml");
    if config_path.exists() {
        println!("notfiles.toml already exists.");
        return Ok(());
    }
    fs::write(&config_path, notcore::config::starter_toml())?;
    println!("Created notfiles.toml");
    Ok(())
}
```

- [ ] **Step 6: Build and test `notfiles`**

```bash
cargo test -p notfiles
```

Expected: all tests pass (same as before the migration).

- [ ] **Step 7: Delete old `src/` and old `Cargo.toml`**

```bash
rm -rf src/ tests/ Cargo.toml.old Cargo.lock.old
```

- [ ] **Step 8: Run full workspace build**

```bash
cargo build --workspace
```

Expected: builds cleanly with no errors.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat(workspace): migrate notfiles into crates/ workspace layout"
```

---

## Task 3: Add `notsecrets` crate

**Files:**
- Create: `crates/notsecrets/Cargo.toml`
- Create: `crates/notsecrets/src/lib.rs`
- Create: `crates/notsecrets/src/sources/mod.rs`
- Create: `crates/notsecrets/src/sources/bitwarden.rs`
- Create: `crates/notsecrets/src/sources/file.rs`
- Create: `crates/notsecrets/src/sources/prompt.rs`
- Create: `crates/notsecrets/tests/integration.rs`

- [ ] **Step 1: Write failing test for `AgeKeySource` trait resolution**

Create `crates/notsecrets/tests/integration.rs`:

```rust
use std::fs;
use tempfile::TempDir;
use notsecrets::{resolve_age_key, AgeKeySource, FileSource};

#[test]
fn test_file_source_reads_key() {
    let dir = TempDir::new().unwrap();
    let key_file = dir.path().join("age.key");
    fs::write(&key_file, "AGE-SECRET-KEY-1ABCDEF\n").unwrap();

    let source = FileSource::new(key_file.clone());
    let result = source.retrieve();
    assert!(result.is_ok());
    assert_eq!(result.unwrap().trim(), "AGE-SECRET-KEY-1ABCDEF");
}

#[test]
fn test_file_source_missing_returns_err() {
    let source = FileSource::new("/nonexistent/age.key".into());
    assert!(source.retrieve().is_err());
}

#[test]
fn test_resolve_age_key_uses_file_fallback() {
    let dir = TempDir::new().unwrap();
    let key_file = dir.path().join("age.key");
    fs::write(&key_file, "AGE-SECRET-KEY-1TEST\n").unwrap();

    // Only FileSource in the chain — should succeed
    let sources: Vec<Box<dyn AgeKeySource>> = vec![
        Box::new(FileSource::new(key_file)),
    ];
    let key = resolve_age_key(sources).unwrap();
    assert_eq!(key.trim(), "AGE-SECRET-KEY-1TEST");
}

#[test]
fn test_resolve_age_key_all_fail_returns_err() {
    let sources: Vec<Box<dyn AgeKeySource>> = vec![
        Box::new(FileSource::new("/nonexistent".into())),
    ];
    assert!(resolve_age_key(sources).is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p notsecrets 2>&1 | head -20
```

Expected: compile error — `notsecrets` crate does not exist yet.

- [ ] **Step 3: Create `crates/notsecrets/Cargo.toml`**

```toml
[package]
name    = "notsecrets"
version = "0.1.0"
edition = "2024"

[dependencies]
notcore    = { workspace = true }
anyhow     = { workspace = true }
thiserror  = { workspace = true }
which      = { workspace = true }
rpassword  = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 4: Create `crates/notsecrets/src/sources/file.rs`**

```rust
use std::path::PathBuf;
use anyhow::Result;
use crate::AgeKeySource;

pub struct FileSource {
    path: PathBuf,
}

impl FileSource {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl AgeKeySource for FileSource {
    fn name(&self) -> &str { "file" }

    fn retrieve(&self) -> Result<String> {
        Ok(std::fs::read_to_string(&self.path)
            .map_err(|e| anyhow::anyhow!("cannot read key file {}: {e}", self.path.display()))?)
    }
}
```

- [ ] **Step 5: Create `crates/notsecrets/src/sources/prompt.rs`**

```rust
use anyhow::Result;
use crate::AgeKeySource;

pub struct PromptSource;

impl AgeKeySource for PromptSource {
    fn name(&self) -> &str { "prompt" }

    fn retrieve(&self) -> Result<String> {
        let key = rpassword::prompt_password("Paste your age private key: ")
            .map_err(|e| anyhow::anyhow!("could not read age key from prompt: {e}"))?;
        if key.trim().is_empty() {
            anyhow::bail!("empty age key entered");
        }
        Ok(key)
    }
}
```

- [ ] **Step 6: Create `crates/notsecrets/src/sources/bitwarden.rs`**

```rust
use anyhow::{bail, Result};
use std::process::Command;
use which::which;
use crate::AgeKeySource;

pub struct BitwardenSource {
    /// Bitwarden item name containing the age key in its `notes` field.
    pub item_name: String,
}

impl BitwardenSource {
    pub fn new(item_name: impl Into<String>) -> Self {
        Self { item_name: item_name.into() }
    }
}

impl AgeKeySource for BitwardenSource {
    fn name(&self) -> &str { "bitwarden" }

    fn retrieve(&self) -> Result<String> {
        if which("bw").is_err() {
            bail!("bw CLI not found in PATH");
        }

        // Check if session is already unlocked (BW_SESSION env var)
        let session = std::env::var("BW_SESSION").unwrap_or_default();
        let session = if session.is_empty() {
            // Prompt for master password
            let password = rpassword::prompt_password("Bitwarden master password: ")
                .map_err(|e| anyhow::anyhow!("could not read password: {e}"))?;
            let output = Command::new("bw")
                .args(["unlock", "--raw", &password])
                .output()?;
            if !output.status.success() {
                bail!("bw unlock failed: {}", String::from_utf8_lossy(&output.stderr));
            }
            String::from_utf8(output.stdout)?.trim().to_string()
        } else {
            session
        };

        let output = Command::new("bw")
            .args(["get", "notes", &self.item_name, "--session", &session])
            .output()?;

        if !output.status.success() {
            bail!(
                "bw get notes '{}' failed: {}",
                self.item_name,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let key = String::from_utf8(output.stdout)?.trim().to_string();
        if key.is_empty() {
            bail!("Bitwarden item '{}' has empty notes", self.item_name);
        }
        Ok(key)
    }
}
```

- [ ] **Step 7: Create `crates/notsecrets/src/sources/mod.rs`**

```rust
pub mod bitwarden;
pub mod file;
pub mod prompt;

pub use bitwarden::BitwardenSource;
pub use file::FileSource;
pub use prompt::PromptSource;
```

- [ ] **Step 8: Create `crates/notsecrets/src/lib.rs`**

```rust
pub mod sources;

pub use sources::{BitwardenSource, FileSource, PromptSource};

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Trait for retrieving an age private key from some source.
pub trait AgeKeySource {
    fn name(&self) -> &str;
    fn retrieve(&self) -> Result<String>;
}

/// Try each source in order; return the first success.
/// Returns an error if all sources fail.
pub fn resolve_age_key(sources: Vec<Box<dyn AgeKeySource>>) -> Result<String> {
    let mut last_err = String::new();
    for source in sources {
        match source.retrieve() {
            Ok(key) => return Ok(key),
            Err(e) => {
                eprintln!("  [{}] {e}", source.name());
                last_err = format!("{e}");
            }
        }
    }
    bail!("all age key sources failed; last error: {last_err}")
}

/// Write the age key to `~/.config/sops/age/keys.txt` (mode 0600).
pub fn install_age_key(key: &str) -> Result<PathBuf> {
    let path = dirs::home_dir()
        .context("cannot find home directory")?
        .join(".config/sops/age/keys.txt");
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, key)?;
    // chmod 600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(path)
}

/// Run `sops --decrypt <sops_file>` and return the decrypted content.
/// Requires `SOPS_AGE_KEY_FILE` env var to be set, or the key installed at the default path.
pub fn decrypt_sops(sops_file: &Path) -> Result<String> {
    let output = Command::new("sops")
        .args(["--decrypt", sops_file.to_str().unwrap()])
        .output()
        .context("failed to run sops")?;

    if !output.status.success() {
        bail!("sops decrypt failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(String::from_utf8(output.stdout)?)
}
```

- [ ] **Step 9: Run tests**

```bash
cargo test -p notsecrets
```

Expected: `test_file_source_reads_key`, `test_file_source_missing_returns_err`, `test_resolve_age_key_uses_file_fallback`, `test_resolve_age_key_all_fail_returns_err` all pass.

- [ ] **Step 10: Commit**

```bash
git add crates/notsecrets/
git commit -m "feat(notsecrets): add age key retrieval with bw/file/prompt sources"
```

---

## Task 4: Add `nothooks` crate

**Files:**
- Create: `crates/nothooks/Cargo.toml`
- Create: `crates/nothooks/src/lib.rs`
- Create: `crates/nothooks/src/main.rs`
- Create: `crates/nothooks/src/runner.rs`
- Create: `crates/nothooks/src/state.rs`
- Create: `crates/nothooks/tests/integration.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/nothooks/tests/integration.rs`:

```rust
use std::fs;
use tempfile::TempDir;
use nothooks::{HookRunner, HookResult};
use notcore::{HookPhase, HookSpec};

fn make_hook_script(dir: &TempDir, name: &str, content: &str) -> HookSpec {
    let path = dir.path().join(format!("{name}.nu"));
    fs::write(&path, content).unwrap();
    HookSpec {
        name: name.to_string(),
        script: path.to_str().unwrap().to_string(),
        phase: HookPhase::Dot,
    }
}

#[test]
fn test_hook_success() {
    let dir = TempDir::new().unwrap();
    let spec = make_hook_script(&dir, "ok-hook", "print hello");
    let runner = HookRunner::new(dir.path().to_path_buf());
    let result = runner.run_hook(&spec);
    assert!(matches!(result, HookResult::Ok));
}

#[test]
fn test_hook_failure() {
    let dir = TempDir::new().unwrap();
    let spec = make_hook_script(&dir, "fail-hook", "exit 1\n");
    let runner = HookRunner::new(dir.path().to_path_buf());
    let result = runner.run_hook(&spec);
    assert!(matches!(result, HookResult::Failed(_)));
}

#[test]
fn test_setup_hook_skipped_on_rerun() {
    let dir = TempDir::new().unwrap();
    let mut spec = make_hook_script(&dir, "setup-hook", "print ran");
    spec.phase = notcore::HookPhase::Setup;

    let runner = HookRunner::new(dir.path().to_path_buf());
    let r1 = runner.run_hook(&spec);
    assert!(matches!(r1, HookResult::Ok));

    // Second run — should be skipped
    let r2 = runner.run_hook(&spec);
    assert!(matches!(r2, HookResult::Skipped));
}

#[test]
fn test_setup_hook_force_reruns() {
    let dir = TempDir::new().unwrap();
    let mut spec = make_hook_script(&dir, "force-hook", "print ran");
    spec.phase = notcore::HookPhase::Setup;

    let runner = HookRunner::new(dir.path().to_path_buf());
    runner.run_hook(&spec);

    let runner2 = HookRunner::with_force(dir.path().to_path_buf());
    let r2 = runner2.run_hook(&spec);
    assert!(matches!(r2, HookResult::Ok));
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p nothooks 2>&1 | head -20
```

Expected: compile error.

- [ ] **Step 3: Create `crates/nothooks/Cargo.toml`**

```toml
[package]
name    = "nothooks"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "nothooks"
path = "src/main.rs"

[lib]
name = "nothooks"
path = "src/lib.rs"

[dependencies]
notcore  = { workspace = true }
anyhow   = { workspace = true }
serde    = { workspace = true }
toml     = { workspace = true }
clap     = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 4: Create `crates/nothooks/src/state.rs`**

```rust
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use anyhow::Result;
use serde::{Deserialize, Serialize};

const STATE_FILE: &str = ".nothooks-state.toml";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct HookState {
    pub completed_setup_hooks: HashSet<String>,
}

impl HookState {
    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join(STATE_FILE);
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        let path = dir.join(STATE_FILE);
        std::fs::write(path, toml::to_string(self)?)?;
        Ok(())
    }

    pub fn mark_done(&mut self, name: &str) {
        self.completed_setup_hooks.insert(name.to_string());
    }

    pub fn is_done(&self, name: &str) -> bool {
        self.completed_setup_hooks.contains(name)
    }
}
```

- [ ] **Step 5: Create `crates/nothooks/src/runner.rs`**

```rust
use std::path::PathBuf;
use std::process::Command;
use notcore::HookSpec;
use crate::HookResult;
use crate::state::HookState;
use notcore::HookPhase;

pub struct HookRunner {
    state_dir: PathBuf,
    force: bool,
}

impl HookRunner {
    pub fn new(state_dir: PathBuf) -> Self {
        Self { state_dir, force: false }
    }

    pub fn with_force(state_dir: PathBuf) -> Self {
        Self { state_dir, force: true }
    }

    pub fn run_hook(&self, spec: &HookSpec) -> HookResult {
        let mut state = HookState::load(&self.state_dir).unwrap_or_default();

        if spec.phase == HookPhase::Setup && !self.force && state.is_done(&spec.name) {
            return HookResult::Skipped;
        }

        let result = Command::new("nu")
            .arg(&spec.script)
            .status();

        match result {
            Ok(status) if status.success() => {
                if spec.phase == HookPhase::Setup {
                    state.mark_done(&spec.name);
                    let _ = state.save(&self.state_dir);
                }
                HookResult::Ok
            }
            Ok(status) => HookResult::Failed(format!("exit code {}", status.code().unwrap_or(-1))),
            Err(e) => HookResult::Failed(e.to_string()),
        }
    }
}
```

- [ ] **Step 6: Create `crates/nothooks/src/lib.rs`**

```rust
pub mod runner;
pub mod state;

pub use runner::HookRunner;
use notcore::{HookPhase, HookSpec, Report, StepStatus};

#[derive(Debug, PartialEq)]
pub enum HookResult {
    Ok,
    Skipped,
    Failed(String),
}

/// Run all hooks matching `phase` and collect into a `Report`.
pub fn run_phase(
    hooks: &[HookSpec],
    phase: &HookPhase,
    runner: &HookRunner,
) -> Report {
    let mut report = Report::default();
    for hook in hooks.iter().filter(|h| &h.phase == phase) {
        let result = runner.run_hook(hook);
        let status = match &result {
            HookResult::Ok => StepStatus::Ok,
            HookResult::Skipped => StepStatus::Skipped,
            HookResult::Failed(msg) => StepStatus::Failed(msg.clone()),
        };
        report.add(&hook.name, status);
    }
    report
}
```

- [ ] **Step 7: Create `crates/nothooks/src/main.rs`**

```rust
use anyhow::Result;
use clap::{Parser, Subcommand};
use nothooks::{HookRunner, run_phase};
use notcore::{HookPhase, HookSpec};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "nothooks", about = "Bootstrap hook runner")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,

    /// Force re-run of setup hooks
    #[arg(long, global = true)]
    force: bool,

    /// Path to hooks config TOML
    #[arg(long, global = true, default_value = "notstrap.toml")]
    config: PathBuf,

    /// Directory for state file (default: current dir)
    #[arg(long, global = true)]
    state_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run hooks for a phase
    Run {
        /// Phase to run: dot or setup
        #[arg(long)]
        phase: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let state_dir = cli.state_dir
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let content = std::fs::read_to_string(&cli.config)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", cli.config.display()))?;

    #[derive(serde::Deserialize)]
    struct HooksFile { hooks: Vec<HookSpec> }
    let file: HooksFile = toml::from_str(&content)?;

    let phase = match cli.command {
        Cmd::Run { ref phase } => match phase.as_str() {
            "dot" => HookPhase::Dot,
            "setup" => HookPhase::Setup,
            other => anyhow::bail!("unknown phase '{other}', use dot or setup"),
        },
    };

    let runner = if cli.force {
        HookRunner::with_force(state_dir)
    } else {
        HookRunner::new(state_dir)
    };

    let report = run_phase(&file.hooks, &phase, &runner);
    report.print();

    if report.has_failures() {
        std::process::exit(1);
    }
    Ok(())
}
```

- [ ] **Step 8: Run tests**

```bash
cargo test -p nothooks
```

Expected: all 4 integration tests pass.

- [ ] **Step 9: Commit**

```bash
git add crates/nothooks/
git commit -m "feat(nothooks): add phase-aware hook runner with setup-hook state tracking"
```

---

## Task 5: Add `notstrap` crate

**Files:**
- Create: `crates/notstrap/Cargo.toml`
- Create: `crates/notstrap/src/main.rs`
- Create: `crates/notstrap/src/prereqs.rs`
- Create: `crates/notstrap/src/repo.rs`

- [ ] **Step 1: Create `crates/notstrap/Cargo.toml`**

```toml
[package]
name    = "notstrap"
version = "0.1.0"
edition = "2024"
description = "New-machine bootstrap orchestrator"

[[bin]]
name = "notstrap"
path = "src/main.rs"

[dependencies]
notcore     = { workspace = true }
notfiles    = { workspace = true }
notsecrets  = { workspace = true }
nothooks    = { workspace = true }
clap        = { workspace = true }
anyhow      = { workspace = true }
which       = { workspace = true }
serde       = { workspace = true }
toml        = { workspace = true }
```

- [ ] **Step 2: Create `crates/notstrap/src/prereqs.rs`**

```rust
use anyhow::Result;
use which::which;

pub struct Prereq {
    pub cmd: &'static str,
    pub install_hint: &'static str,
}

const PREREQS: &[Prereq] = &[
    Prereq { cmd: "nu",   install_hint: "brew install nushell  OR  nix-env -iA nixpkgs.nushell" },
    Prereq { cmd: "sops", install_hint: "brew install sops  OR  nix-env -iA nixpkgs.sops" },
    Prereq { cmd: "age",  install_hint: "brew install age   OR  nix-env -iA nixpkgs.age" },
];

/// Check for required tools. Returns list of missing tools with hints.
pub fn check_prerequisites() -> Result<()> {
    let missing: Vec<&Prereq> = PREREQS.iter().filter(|p| which(p.cmd).is_err()).collect();
    if missing.is_empty() {
        return Ok(());
    }
    eprintln!("Missing required tools:\n");
    for p in &missing {
        eprintln!("  {} — {}", p.cmd, p.install_hint);
    }
    anyhow::bail!("{} prerequisite(s) missing", missing.len())
}
```

- [ ] **Step 3: Create `crates/notstrap/src/repo.rs`**

```rust
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Clone `url` to `dest` if `dest` doesn't already exist.
pub fn clone_if_missing(url: &str, dest: &Path) -> Result<bool> {
    if dest.exists() {
        return Ok(false);
    }
    let status = Command::new("git")
        .args(["clone", url, dest.to_str().unwrap()])
        .status()
        .context("failed to run git clone")?;
    if !status.success() {
        anyhow::bail!("git clone {} failed", url);
    }
    Ok(true)
}
```

- [ ] **Step 4: Create `crates/notstrap/src/main.rs`**

```rust
use anyhow::{Context, Result};
use clap::Parser;
use notcore::{HookPhase, Report, StepStatus};
use notfiles::{link, LinkOptions};
use nothooks::{run_phase, HookRunner};
use notsecrets::{
    install_age_key, resolve_age_key, BitwardenSource, FileSource, PromptSource,
};
use std::path::PathBuf;
use serde::Deserialize;

#[derive(Parser)]
#[command(name = "notstrap", about = "Bootstrap a new machine from dotfiles")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(clap::Subcommand)]
enum Cmd {
    /// Run the full bootstrap sequence
    Run {
        /// Path to notstrap.toml config
        #[arg(long, default_value = "notstrap.toml")]
        config: PathBuf,

        /// Force re-run of setup hooks
        #[arg(long)]
        force: bool,

        /// Path to age key file (skips Bitwarden and prompt)
        #[arg(long)]
        key_file: Option<PathBuf>,

        /// Path to dotfiles directory (default: ~/dotfiles)
        #[arg(long)]
        dotfiles: Option<PathBuf>,
    },
}

#[derive(Deserialize)]
struct NotstrapConfig {
    bootstrap: BootstrapSection,
    #[serde(default)]
    hooks: Vec<notcore::HookSpec>,
}

#[derive(Deserialize)]
struct BootstrapSection {
    dotfiles_repo: String,
    dotfiles_dir: String,
    #[serde(default = "default_bw_item")]
    bw_age_item: String,
    #[serde(default = "default_sops_file")]
    sops_file: String,
}

fn default_bw_item() -> String { "age-key-dotfiles".to_string() }
fn default_sops_file() -> String { "secrets/bootstrap.sops.env".to_string() }

fn main() -> Result<()> {
    let cli = Cli::parse();
    let Cmd::Run { config, force, key_file, dotfiles } = cli.command;

    let mut report = Report::default();

    // 1. Prerequisites
    print!("Checking prerequisites... ");
    match crate::prereqs::check_prerequisites() {
        Ok(_) => { println!("ok"); report.add("prerequisites", StepStatus::Ok); }
        Err(e) => {
            println!("FAILED");
            report.add("prerequisites", StepStatus::Failed(e.to_string()));
            report.print();
            std::process::exit(1);
        }
    }

    // 2. Load config
    let config_content = std::fs::read_to_string(&config)
        .with_context(|| format!("cannot read {}", config.display()))?;
    let cfg: NotstrapConfig = toml::from_str(&config_content)?;

    let dotfiles_dir = dotfiles.unwrap_or_else(|| {
        notcore::expand_tilde(&cfg.bootstrap.dotfiles_dir).unwrap()
    });

    // 3. Clone dotfiles if missing
    match crate::repo::clone_if_missing(&cfg.bootstrap.dotfiles_repo, &dotfiles_dir) {
        Ok(true)  => { println!("Cloned dotfiles."); report.add("clone dotfiles", StepStatus::Ok); }
        Ok(false) => { report.add("clone dotfiles", StepStatus::Skipped); }
        Err(e)    => {
            report.add("clone dotfiles", StepStatus::Failed(e.to_string()));
            report.print();
            std::process::exit(1);
        }
    }

    // 4. Retrieve age key and decrypt secrets
    print!("Retrieving age key... ");
    let sources: Vec<Box<dyn notsecrets::AgeKeySource>> = if let Some(kf) = key_file {
        vec![Box::new(FileSource::new(kf))]
    } else {
        vec![
            Box::new(BitwardenSource::new(&cfg.bootstrap.bw_age_item)),
            Box::new(FileSource::new(
                notsecrets::install_age_key("").map(|_| PathBuf::new()).unwrap_or_default()
            )),
            Box::new(PromptSource),
        ]
    };

    match resolve_age_key(sources) {
        Ok(key) => {
            install_age_key(&key)?;
            println!("ok");
            report.add("age key", StepStatus::Ok);
        }
        Err(e) => {
            println!("FAILED");
            report.add("age key", StepStatus::Failed(e.to_string()));
            report.print();
            std::process::exit(1);
        }
    }

    // 5. Decrypt sops secrets
    let sops_path = dotfiles_dir.join(&cfg.bootstrap.sops_file);
    match notsecrets::decrypt_sops(&sops_path) {
        Ok(env_content) => {
            // Inject decrypted vars into current process environment
            for line in env_content.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    let k = k.trim();
                    let v = v.trim().trim_matches('"');
                    if !k.is_empty() && !k.starts_with('#') {
                        std::env::set_var(k, v);
                    }
                }
            }
            report.add("decrypt secrets", StepStatus::Ok);
        }
        Err(e) => {
            report.add("decrypt secrets", StepStatus::Failed(e.to_string()));
            report.print();
            std::process::exit(1);
        }
    }

    // 6. Link dotfiles
    let opts = LinkOptions { force: false, no_backup: false, dry_run: false, verbose: false };
    match link(&dotfiles_dir, &[], &opts) {
        Ok(state) => {
            let count = state.entries.len();
            println!("Linked {count} files.");
            report.add(format!("link dotfiles ({count} files)"), StepStatus::Ok);
        }
        Err(e) => {
            report.add("link dotfiles", StepStatus::Failed(e.to_string()));
        }
    }

    // 7. Run hooks
    let runner = if force {
        HookRunner::with_force(dotfiles_dir.clone())
    } else {
        HookRunner::new(dotfiles_dir.clone())
    };

    for (phase, label) in [(HookPhase::Dot, "dot hooks"), (HookPhase::Setup, "setup hooks")] {
        let phase_report = run_phase(&cfg.hooks, &phase, &runner);
        let ok = phase_report.steps.iter().filter(|s| matches!(s.status, notcore::StepStatus::Ok)).count();
        let skipped = phase_report.steps.iter().filter(|s| matches!(s.status, notcore::StepStatus::Skipped)).count();
        let failed = phase_report.steps.iter().filter(|s| matches!(s.status, notcore::StepStatus::Failed(_))).count();
        let summary = if failed > 0 {
            StepStatus::Failed(format!("{failed} failed"))
        } else {
            StepStatus::Ok
        };
        let _ = (ok, skipped);
        report.add(label, summary);
        phase_report.print();
    }

    // 8. Final report
    println!("\n── Bootstrap complete ──");
    report.print();

    if report.has_failures() {
        std::process::exit(1);
    }
    Ok(())
}

mod prereqs;
mod repo;
```

- [ ] **Step 5: Build notstrap**

```bash
cargo build -p notstrap
```

Expected: builds cleanly.

- [ ] **Step 6: Run full workspace test suite**

```bash
cargo test --workspace
```

Expected: all tests pass across all crates.

- [ ] **Step 7: Commit**

```bash
git add crates/notstrap/
git commit -m "feat(notstrap): add new-machine bootstrap orchestrator"
```

---

## Task 6: Cleanup and workspace polish

**Files:**
- Delete: `Cargo.toml.old`, `Cargo.lock.old` (if not already removed)
- Modify: `Cargo.toml` (workspace root) — add `[profile]` settings

- [ ] **Step 1: Add release profile to workspace Cargo.toml**

Add to the end of `Cargo.toml`:

```toml
[profile.release]
opt-level = 3
lto = "thin"
strip = true
```

- [ ] **Step 2: Run clippy across workspace**

```bash
cargo clippy --workspace -- -D warnings
```

Fix any warnings before continuing.

- [ ] **Step 3: Run full test suite one final time**

```bash
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "chore(workspace): add release profile, clean up old files"
```

---

## Self-Review Notes

**Spec coverage:**
- ✓ notcore (error, config, paths, types)
- ✓ notfiles migrated to workspace crate with lib + bin
- ✓ notsecrets with bw/file/prompt sources, AgeKeySource trait, install_age_key, decrypt_sops
- ✓ nothooks with dot/setup phases, state persistence, force flag
- ✓ notstrap orchestrating all crates in correct order
- ✓ Migration path (old src/ deleted after crates/ established)

**Type consistency check:**
- `LinkOptions` used in Task 2 lib.rs and Task 5 main.rs ✓
- `HookRunner::new` / `HookRunner::with_force` defined in Task 4 and used in Task 5 ✓
- `AgeKeySource` trait defined in notsecrets lib.rs and used in notstrap ✓
- `Report` / `StepStatus` from notcore used consistently ✓
- `HookPhase::Dot` / `HookPhase::Setup` from notcore used consistently ✓
- `state.entries` referenced in notstrap Task 5 — `State.entries` field is public in linker.rs ✓ (verify during implementation)

**One known item to verify during Task 2:** the existing `linker.rs` and `status.rs` have many internal `use crate::` references — Step 3 of Task 2 calls out the pattern but the implementor should do a full `grep 'use crate::' crates/notfiles/src/` pass after copying files to catch any missed imports.
