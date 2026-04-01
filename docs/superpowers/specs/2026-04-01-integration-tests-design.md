---
title: Integration Tests — notstrap + cross-crate
date: 2026-04-01
status: approved
---

# Integration Tests Design

## Goal

Validate the layered workspace architecture end-to-end: `notstrap` orchestrating `notfiles`, `nothooks`, and `notsecrets` against real temp directories with real Nu scripts and a `FileSource` age key. Catch regressions across crate boundaries that unit tests cannot.

---

## Part 1: notstrap library extraction

### Problem

All orchestration logic lives in `crates/notstrap/src/main.rs`. There is no library surface — nothing to import in tests. `crates/notstrap/src/lib.rs` is a placeholder.

### Change

Move orchestration logic from `main()` into `lib.rs`. Public surface:

```rust
pub struct BootstrapOptions {
    pub config: PathBuf,
    pub force: bool,
    pub key_file: Option<PathBuf>,
    pub dotfiles: Option<PathBuf>,
    /// Optional override for the sops decrypt step.
    /// None = skip env injection (used in tests).
    /// Some(f) = call f(sops_path) and inject the result into env.
    pub env_injector: Option<Box<dyn Fn(&Path) -> anyhow::Result<String>>>,
}

pub fn run(opts: BootstrapOptions) -> anyhow::Result<Report>
```

`main()` becomes:

```rust
fn main() -> Result<()> {
    let cli = Cli::parse();
    let Cmd::Run { config, force, key_file, dotfiles } = cli.command;
    let opts = BootstrapOptions {
        config, force, key_file, dotfiles,
        env_injector: Some(Box::new(|p| notsecrets::decrypt_sops(p))),
    };
    let report = notstrap::run(opts)?;
    if report.has_failures() { std::process::exit(1); }
    Ok(())
}
```

`NotstrapConfig`, `BootstrapSection`, `prereqs`, and `repo` modules stay in `notstrap` (not moved to notcore — they are bootstrap-specific).

The `[lib]` section in `crates/notstrap/Cargo.toml` stays; the placeholder comment in `lib.rs` is replaced with real code.

---

## Part 2: Workspace-level integration test crate

### Location

```
tests/
└── integration/
    ├── Cargo.toml
    └── tests/
        ├── bootstrap.rs
        └── cross_crate.rs
```

Not under `crates/` — this is a test-only crate at the workspace root level. Added to `members` in root `Cargo.toml`.

### Cargo.toml

```toml
[package]
name = "integration"
version = "0.1.0"
edition = "2024"
license.workspace = true
publish = false

[dev-dependencies]
notstrap   = { path = "../../crates/notstrap" }
notfiles   = { path = "../../crates/notfiles" }
nothooks   = { path = "../../crates/nothooks" }
notsecrets = { path = "../../crates/notsecrets" }
notcore    = { path = "../../crates/notcore" }
tempfile   = { workspace = true }
assert_fs  = { workspace = true }
anyhow     = { workspace = true }
```

No `[lib]` or `[[bin]]` — test-only crate. Run with `cargo nextest run -p integration`.

---

## Part 3: Test scenarios

### `tests/bootstrap.rs` — full flow via `notstrap::run()`

All tests use a helper `fn make_dotfiles_dir() -> TempDir` that creates:
- A valid `notfiles.toml` (symlink method, target = tempdir home)
- A `notstrap.toml` with a fake `dotfiles_repo` and the tempdir as `dotfiles_dir`
- A `scripts/` directory with `.nu` hook scripts
- A `age.key` file with a fake key string

**Test 1: `test_full_bootstrap_dot_hooks_only`**

Setup: dotfiles dir with one dot hook (`print hello`) and one file to link. Pass `key_file` pointing to `age.key`, `env_injector: None`.

Assert:
- `Report` has no failures
- The dot hook ran (hook state not written — dot hooks don't track)
- The target file is symlinked into the temp home dir

**Test 2: `test_setup_hooks_skipped_on_rerun`**

Setup: dotfiles dir with one setup hook. Run bootstrap twice with same `dotfiles_dir`.

Assert:
- First run: setup hook `StepStatus::Ok`
- Second run: setup hook `StepStatus::Skipped`
- `.nothooks-state.toml` exists after first run

**Test 3: `test_bootstrap_fails_fast_on_bad_key`**

Setup: `key_file` points to a non-existent path.

Assert:
- `Report` has a failure on the `"age key"` step
- No files linked (bootstrap stopped before step 6)

---

### `tests/cross_crate.rs` — crate boundary tests

**Test 4: `test_nothooks_notsecrets_independent`**

Exercise the `notsecrets → nothooks` boundary without going through notstrap.

Setup:
- Write an age key to a tempfile via `FileSource`
- Resolve it with `resolve_age_key()`
- Write a `.nu` hook script that checks `$env.TEST_KEY` is set (set manually in test env)
- Run it via `HookRunner`

Assert: `HookResult::Ok`

**Test 5: `test_notfiles_respects_default_ignore`**

Exercise the `notcore` ignore list fix (from audit finding).

Setup:
- Create a package dir containing `foo.txt`, `.notfiles-state.toml`, `.nothooks-state.toml`
- Run `notfiles::link()` targeting a temp home dir

Assert:
- `foo.txt` is linked
- `.notfiles-state.toml` is NOT linked
- `.nothooks-state.toml` is NOT linked

---

## Running tests

```bash
# Run integration tests only
cargo nextest run -p integration

# Run all workspace tests
cargo nextest run --workspace
```

---

## Non-goals

- No sops binary required — `env_injector: None` in all tests
- No Bitwarden CLI required — all tests use `FileSource`
- No network access
- No testing of `notstrap`'s CLI arg parsing (that's thin and not worth testing)
