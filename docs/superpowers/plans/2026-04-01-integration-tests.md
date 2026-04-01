# Integration Tests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract `notstrap` orchestration into a testable library, then add a workspace-level integration test crate exercising the full cross-crate bootstrap flow.

**Architecture:** `notstrap::run(BootstrapOptions)` becomes the public entry point; `main()` becomes a thin shim. A new `tests/integration/` workspace crate imports all feature crates and runs 5 tests via `cargo nextest`.

**Tech Stack:** Rust edition 2024, `tempfile`, `assert_fs`, `cargo-nextest`

---

## File Map

| Action | Path | Responsibility |
|--------|------|----------------|
| Modify | `crates/notstrap/src/lib.rs` | `BootstrapOptions`, `run()`, config types |
| Modify | `crates/notstrap/src/main.rs` | Thin CLI shim only |
| Modify | `crates/notstrap/Cargo.toml` | Add `[lib]` section |
| Create | `tests/integration/Cargo.toml` | Test-only crate manifest |
| Create | `tests/integration/tests/bootstrap.rs` | 3 full-flow tests via `notstrap::run()` |
| Create | `tests/integration/tests/cross_crate.rs` | 2 cross-crate boundary tests |
| Modify | `Cargo.toml` (workspace root) | Add `tests/integration` to `members` |

---

## Task 1: Extract notstrap orchestration into lib.rs

**Files:**
- Modify: `crates/notstrap/src/lib.rs`
- Modify: `crates/notstrap/src/main.rs`
- Modify: `crates/notstrap/Cargo.toml`

- [ ] **Step 1: Add `[lib]` section to notstrap Cargo.toml**

In `crates/notstrap/Cargo.toml`, add after the existing `[[bin]]` block:

```toml
[lib]
name = "notstrap"
path = "src/lib.rs"
```

- [ ] **Step 2: Write lib.rs with BootstrapOptions and run()**

Replace `crates/notstrap/src/lib.rs` (currently just `// placeholder`) with:

```rust
use anyhow::{Context, Result};
use notcore::{HookPhase, Report, StepStatus};
use notfiles::{link, LinkOptions};
use nothooks::{run_phase, HookRunner};
use notsecrets::{install_age_key, resolve_age_key, FileSource, BitwardenSource, PromptSource};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub mod prereqs;
pub mod repo;

#[derive(Deserialize)]
pub struct NotstrapConfig {
    pub bootstrap: BootstrapSection,
    #[serde(default)]
    pub hooks: Vec<notcore::HookSpec>,
}

#[derive(Deserialize)]
pub struct BootstrapSection {
    pub dotfiles_repo: String,
    pub dotfiles_dir: String,
    #[serde(default = "default_bw_item")]
    pub bw_age_item: String,
    #[serde(default = "default_sops_file")]
    pub sops_file: String,
}

pub fn default_bw_item() -> String { "age-key-dotfiles".to_string() }
pub fn default_sops_file() -> String { "secrets/bootstrap.sops.env".to_string() }

pub struct BootstrapOptions {
    pub config: PathBuf,
    pub force: bool,
    pub key_file: Option<PathBuf>,
    pub dotfiles: Option<PathBuf>,
    /// None = skip prereq check (tests). Some(f) = run f().
    pub check_prereqs: Option<Box<dyn Fn() -> Result<()>>>,
    /// None = skip env injection (tests). Some(f) = decrypt sops at path and inject.
    pub env_injector: Option<Box<dyn Fn(&Path) -> Result<String>>>,
}

pub fn run(opts: BootstrapOptions) -> Result<Report> {
    let mut report = Report::default();

    // 1. Prerequisites
    if let Some(check) = opts.check_prereqs {
        match check() {
            Ok(_) => { report.add("prerequisites", StepStatus::Ok); }
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

    let dotfiles_dir = opts.dotfiles.unwrap_or_else(|| {
        notcore::expand_tilde(&cfg.bootstrap.dotfiles_dir).unwrap()
    });

    // 3. Clone dotfiles if missing
    match repo::clone_if_missing(&cfg.bootstrap.dotfiles_repo, &dotfiles_dir) {
        Ok(true)  => { report.add("clone dotfiles", StepStatus::Ok); }
        Ok(false) => { report.add("clone dotfiles", StepStatus::Skipped); }
        Err(e)    => {
            report.add("clone dotfiles", StepStatus::Failed(e.to_string()));
            return Ok(report);
        }
    }

    // 4. Retrieve age key and install
    let sources: Vec<Box<dyn notsecrets::AgeKeySource>> = if let Some(kf) = opts.key_file {
        vec![Box::new(FileSource::new(kf))]
    } else {
        vec![
            Box::new(BitwardenSource::new(&cfg.bootstrap.bw_age_item)),
            Box::new(PromptSource),
        ]
    };

    match resolve_age_key(sources) {
        Ok(key) => {
            install_age_key(&key)?;
            report.add("age key", StepStatus::Ok);
        }
        Err(e) => {
            report.add("age key", StepStatus::Failed(e.to_string()));
            return Ok(report);
        }
    }

    // 5. Decrypt sops secrets (optional)
    if let Some(injector) = opts.env_injector {
        let sops_path = dotfiles_dir.join(&cfg.bootstrap.sops_file);
        match injector(&sops_path) {
            Ok(env_content) => {
                for line in env_content.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        let k = k.trim();
                        let v = v.trim().trim_matches('"');
                        if !k.is_empty() && !k.starts_with('#') {
                            // Safety: single-threaded bootstrap, no concurrent env readers
                            unsafe { std::env::set_var(k, v); }
                        }
                    }
                }
                report.add("decrypt secrets", StepStatus::Ok);
            }
            Err(e) => {
                report.add("decrypt secrets", StepStatus::Failed(e.to_string()));
                return Ok(report);
            }
        }
    }

    // 6. Link dotfiles
    let link_opts = LinkOptions { force: opts.force, no_backup: false, dry_run: false, verbose: false };
    match link(&dotfiles_dir, &[], &link_opts) {
        Ok(state) => {
            let count = state.entries.len();
            report.add(format!("link dotfiles ({count} files)"), StepStatus::Ok);
        }
        Err(e) => {
            report.add("link dotfiles", StepStatus::Failed(e.to_string()));
        }
    }

    // 7. Run hooks
    let runner = if opts.force {
        HookRunner::with_force(dotfiles_dir.clone())
    } else {
        HookRunner::new(dotfiles_dir.clone())
    };

    for (phase, label) in [(HookPhase::Dot, "dot hooks"), (HookPhase::Setup, "setup hooks")] {
        let phase_report = run_phase(&cfg.hooks, &phase, &runner);
        let failed = phase_report.steps.iter()
            .filter(|s| matches!(s.status, notcore::StepStatus::Failed(_)))
            .count();
        let summary = if failed > 0 {
            StepStatus::Failed(format!("{failed} failed"))
        } else {
            StepStatus::Ok
        };
        report.add(label, summary);
    }

    Ok(report)
}
```

- [ ] **Step 3: Slim down main.rs to a thin shim**

Replace the full contents of `crates/notstrap/src/main.rs` with:

```rust
use anyhow::Result;
use clap::Parser;
use notstrap::{BootstrapOptions, prereqs, run};

#[derive(Parser)]
#[command(name = "notstrap", about = "Bootstrap a new machine from dotfiles")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(clap::Subcommand)]
enum Cmd {
    Run {
        #[arg(long, default_value = "notstrap.toml")]
        config: std::path::PathBuf,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        key_file: Option<std::path::PathBuf>,
        #[arg(long)]
        dotfiles: Option<std::path::PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let Cmd::Run { config, force, key_file, dotfiles } = cli.command;
    let opts = BootstrapOptions {
        config,
        force,
        key_file,
        dotfiles,
        check_prereqs: Some(Box::new(prereqs::check_prerequisites)),
        env_injector: Some(Box::new(|p| notsecrets::decrypt_sops(p))),
    };
    let report = run(opts)?;
    report.print();
    if report.has_failures() {
        std::process::exit(1);
    }
    Ok(())
}
```

- [ ] **Step 4: Verify it compiles**

```bash
cargo check -p notstrap
```

Expected: no errors.

- [ ] **Step 5: Run existing tests**

```bash
cargo nextest run --workspace
```

Expected: all tests pass (same count as before).

- [ ] **Step 6: Commit**

```bash
git add crates/notstrap/
git commit -m "refactor(notstrap): extract run() into lib with BootstrapOptions"
```

---

## Task 2: Create the integration test crate

**Files:**
- Create: `tests/integration/Cargo.toml`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create tests/integration/Cargo.toml**

```toml
[package]
name = "integration"
version = "0.1.0"
edition = "2024"
license.workspace = true
publish = false

[dev-dependencies]
notstrap  = { path = "../../crates/notstrap" }
notfiles  = { path = "../../crates/notfiles" }
nothooks  = { path = "../../crates/nothooks" }
notsecrets = { path = "../../crates/notsecrets" }
notcore   = { path = "../../crates/notcore" }
tempfile  = { workspace = true }
assert_fs = { workspace = true }
anyhow    = { workspace = true }
```

- [ ] **Step 2: Add to workspace members**

In root `Cargo.toml`, add `"tests/integration"` to the `members` array:

```toml
[workspace]
members = [
    "crates/notcore",
    "crates/notfiles",
    "crates/notsecrets",
    "crates/nothooks",
    "crates/notstrap",
    "tests/integration",
]
```

- [ ] **Step 3: Verify the crate is recognized**

```bash
cargo check -p integration
```

Expected: no errors (empty crate compiles fine).

- [ ] **Step 4: Commit**

```bash
git add tests/ Cargo.toml Cargo.lock
git commit -m "chore: add integration test crate to workspace"
```

---

## Task 3: Write bootstrap.rs tests (full flow)

**Files:**
- Create: `tests/integration/tests/bootstrap.rs`

These tests exercise `notstrap::run()` against a real temp directory. They require `nu` to be installed (hook scripts are real `.nu` files). They skip prereqs and sops.

Helper used by all three tests — define at the top of the file:

```rust
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use notstrap::{BootstrapOptions, run};
use notcore::StepStatus;

struct TestEnv {
    dotfiles: TempDir,
    home: TempDir,
    config: PathBuf,
    key_file: PathBuf,
}

fn make_test_env() -> TestEnv {
    let dotfiles = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let d = dotfiles.path();

    // age key file
    let key_file = d.join("age.key");
    fs::write(&key_file, "AGE-SECRET-KEY-1TESTKEY\n").unwrap();

    // notfiles.toml — one package "dotfiles" targeting home tempdir
    fs::write(d.join("notfiles.toml"), format!(
        "[defaults]\nmethod = \"symlink\"\ntarget = \"{}\"\n",
        home.path().display()
    )).unwrap();

    // a file to link inside a package dir
    let pkg = d.join("shell");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(pkg.join(".zshrc"), "# test zshrc\n").unwrap();

    // scripts dir for hooks
    fs::create_dir_all(d.join("scripts")).unwrap();

    // notstrap.toml — fake repo (won't clone since dir already exists)
    let config = d.join("notstrap.toml");
    fs::write(&config, format!(
        "[bootstrap]\ndotfiles_repo = \"https://example.com/fake.git\"\ndotfiles_dir = \"{}\"\n",
        d.display()
    )).unwrap();

    TestEnv { dotfiles, home, config, key_file }
}
```

- [ ] **Step 1: Write test_full_bootstrap_dot_hooks_only**

```rust
#[test]
fn test_full_bootstrap_dot_hooks_only() {
    let env = make_test_env();
    let d = env.dotfiles.path();

    // Add a dot hook
    let script = d.join("scripts/greet.nu");
    fs::write(&script, "print hello\n").unwrap();

    // Append hook to notstrap.toml
    fs::write(&env.config, format!(
        "[bootstrap]\ndotfiles_repo = \"https://example.com/fake.git\"\ndotfiles_dir = \"{dotfiles}\"\n\n\
         [[hooks]]\nname = \"greet\"\nscript = \"{script}\"\nphase = \"dot\"\n",
        dotfiles = d.display(),
        script = script.display(),
    )).unwrap();

    let opts = BootstrapOptions {
        config: env.config.clone(),
        force: false,
        key_file: Some(env.key_file.clone()),
        dotfiles: Some(d.to_path_buf()),
        check_prereqs: None,
        env_injector: None,
    };

    let report = run(opts).unwrap();
    assert!(!report.has_failures(), "unexpected failures: {report:?}");

    // dot hook ran
    let greet_step = report.steps.iter().find(|s| s.name == "greet");
    assert!(greet_step.is_some());
    assert_eq!(greet_step.unwrap().status, StepStatus::Ok);

    // .zshrc was linked
    let link_target = env.home.path().join(".zshrc");
    assert!(link_target.exists(), ".zshrc should be linked into home");
    assert!(link_target.is_symlink(), ".zshrc should be a symlink");
}
```

- [ ] **Step 2: Run test to verify it passes**

```bash
cargo nextest run -p integration --test bootstrap test_full_bootstrap_dot_hooks_only
```

Expected: PASS

- [ ] **Step 3: Write test_setup_hooks_skipped_on_rerun**

```rust
#[test]
fn test_setup_hooks_skipped_on_rerun() {
    let env = make_test_env();
    let d = env.dotfiles.path();

    let script = d.join("scripts/setup.nu");
    fs::write(&script, "print setup ran\n").unwrap();

    let config_content = format!(
        "[bootstrap]\ndotfiles_repo = \"https://example.com/fake.git\"\ndotfiles_dir = \"{dotfiles}\"\n\n\
         [[hooks]]\nname = \"install-tools\"\nscript = \"{script}\"\nphase = \"setup\"\n",
        dotfiles = d.display(),
        script = script.display(),
    );
    fs::write(&env.config, &config_content).unwrap();

    let make_opts = |force: bool| BootstrapOptions {
        config: env.config.clone(),
        force,
        key_file: Some(env.key_file.clone()),
        dotfiles: Some(d.to_path_buf()),
        check_prereqs: None,
        env_injector: None,
    };

    // First run — setup hook should run
    let r1 = run(make_opts(false)).unwrap();
    let step1 = r1.steps.iter().find(|s| s.name == "install-tools").unwrap();
    assert_eq!(step1.status, StepStatus::Ok, "first run should run setup hook");

    // State file should exist
    assert!(d.join(".nothooks-state.toml").exists());

    // Second run — setup hook should be skipped
    let r2 = run(make_opts(false)).unwrap();
    let step2 = r2.steps.iter().find(|s| s.name == "install-tools").unwrap();
    assert_eq!(step2.status, StepStatus::Skipped, "second run should skip setup hook");
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo nextest run -p integration --test bootstrap test_setup_hooks_skipped_on_rerun
```

Expected: PASS

- [ ] **Step 5: Write test_bootstrap_fails_fast_on_bad_key**

```rust
#[test]
fn test_bootstrap_fails_fast_on_bad_key() {
    let env = make_test_env();
    let d = env.dotfiles.path();

    let opts = BootstrapOptions {
        config: env.config.clone(),
        force: false,
        key_file: Some("/nonexistent/no-such-key.age".into()),
        dotfiles: Some(d.to_path_buf()),
        check_prereqs: None,
        env_injector: None,
    };

    let report = run(opts).unwrap();

    // age key step must fail
    let key_step = report.steps.iter().find(|s| s.name == "age key").unwrap();
    assert!(matches!(key_step.status, StepStatus::Failed(_)), "bad key path should fail age key step");

    // no link step should appear (stopped early)
    let has_link = report.steps.iter().any(|s| s.name.starts_with("link dotfiles"));
    assert!(!has_link, "should not reach link step after key failure");
}
```

- [ ] **Step 6: Run test to verify it passes**

```bash
cargo nextest run -p integration --test bootstrap test_bootstrap_fails_fast_on_bad_key
```

Expected: PASS

- [ ] **Step 7: Run all bootstrap tests together**

```bash
cargo nextest run -p integration --test bootstrap
```

Expected: 3 passed

- [ ] **Step 8: Commit**

```bash
git add tests/integration/tests/bootstrap.rs
git commit -m "test(integration): add bootstrap flow tests via notstrap::run()"
```

---

## Task 4: Write cross_crate.rs tests

**Files:**
- Create: `tests/integration/tests/cross_crate.rs`

- [ ] **Step 1: Write test_nothooks_notsecrets_independent**

```rust
use std::fs;
use tempfile::TempDir;
use notsecrets::{resolve_age_key, AgeKeySource, FileSource};
use nothooks::{HookResult, HookRunner};
use notcore::{HookPhase, HookSpec};

#[test]
fn test_nothooks_notsecrets_independent() {
    let dir = TempDir::new().unwrap();

    // Write a fake age key via FileSource
    let key_path = dir.path().join("age.key");
    fs::write(&key_path, "AGE-SECRET-KEY-1CROSSCRATE\n").unwrap();

    let sources: Vec<Box<dyn AgeKeySource>> = vec![Box::new(FileSource::new(key_path))];
    let key = resolve_age_key(sources).unwrap();
    assert!(key.trim().starts_with("AGE-SECRET-KEY-"));

    // Write a hook that just prints (doesn't depend on the key value, just proves the chain works)
    let script = dir.path().join("chain.nu");
    fs::write(&script, "print chain-ok\n").unwrap();

    let spec = HookSpec {
        name: "chain".to_string(),
        script: script.to_str().unwrap().to_string(),
        phase: HookPhase::Dot,
    };

    let runner = HookRunner::new(dir.path().to_path_buf());
    let result = runner.run_hook(&spec);
    assert!(matches!(result, HookResult::Ok));
}
```

- [ ] **Step 2: Run test to verify it passes**

```bash
cargo nextest run -p integration --test cross_crate test_nothooks_notsecrets_independent
```

Expected: PASS

- [ ] **Step 3: Write test_notfiles_respects_default_ignore**

```rust
use std::fs;
use tempfile::TempDir;
use notfiles::{link, LinkOptions};

#[test]
fn test_notfiles_respects_default_ignore() {
    let dotfiles = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let d = dotfiles.path();

    // notfiles.toml — package "pkg" targeting home tempdir
    fs::write(d.join("notfiles.toml"), format!(
        "[defaults]\nmethod = \"symlink\"\ntarget = \"{}\"\n",
        home.path().display()
    )).unwrap();

    // Package dir with a normal file and two state files that should be ignored
    let pkg = d.join("pkg");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(pkg.join("foo.txt"), "hello\n").unwrap();
    fs::write(pkg.join(".notfiles-state.toml"), "# state\n").unwrap();
    fs::write(pkg.join(".nothooks-state.toml"), "# state\n").unwrap();

    let opts = LinkOptions { force: false, no_backup: false, dry_run: false, verbose: false };
    let state = link(d, &[], &opts).unwrap();

    // foo.txt linked
    assert!(home.path().join("foo.txt").exists(), "foo.txt should be linked");

    // state files NOT linked
    assert!(!home.path().join(".notfiles-state.toml").exists(),
        ".notfiles-state.toml must not be linked");
    assert!(!home.path().join(".nothooks-state.toml").exists(),
        ".nothooks-state.toml must not be linked");

    // state doesn't record them either
    let names: Vec<_> = state.entries.iter().map(|e| e.source.file_name().unwrap().to_str().unwrap()).collect();
    assert!(!names.contains(&".notfiles-state.toml"));
    assert!(!names.contains(&".nothooks-state.toml"));
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo nextest run -p integration --test cross_crate test_notfiles_respects_default_ignore
```

Expected: PASS

- [ ] **Step 5: Run all integration tests**

```bash
cargo nextest run -p integration
```

Expected: 5 passed

- [ ] **Step 6: Run full workspace to confirm nothing broken**

```bash
cargo nextest run --workspace
```

Expected: all tests pass (existing count + 5 new).

- [ ] **Step 7: Commit**

```bash
git add tests/integration/tests/cross_crate.rs
git commit -m "test(integration): add cross-crate boundary tests"
```

---

## Task 5: Final cleanup and push

- [ ] **Step 1: Run clippy on the whole workspace**

```bash
cargo clippy --workspace -- -D warnings
```

Fix any warnings before proceeding.

- [ ] **Step 2: Push to both remotes**

```bash
git push gitea main && git push github main
```

- [ ] **Step 3: Mark doob todo as complete**

```bash
doob todo complete 9  # "Write integration tests covering the full new-machine bootstrap flow"
```
