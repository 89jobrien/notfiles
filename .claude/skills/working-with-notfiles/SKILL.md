---
name: docs
description: notfiles workspace documentation — architecture, design specs, CI pipeline, configuration patterns, and implementation plans for a 7-crate Rust dotfiles manager
doc_version: 2026-07-15
---

# notfiles Documentation Skill

## Description

This skill synthesizes project documentation extracted from the `notfiles` workspace — a pure-Rust dotfiles manager and new-machine bootstrap system. Sources include architecture decision records, design specs, implementation plans, CI pipeline docs, and configuration references.

**Workspace path:** `/Users/joe/dev/notfiles`  
**Documentation files:** 29 across 4 categories  
**Sources:** High-confidence markdown documentation + configuration analysis  
**Crates covered:** `notcore`, `notfiles`, `notsecrets`, `nothooks`, `notstrap`, `notnet`, `notgraph`

---

## When to Use This Skill

Use this skill when you need to:

- **Understand workspace architecture** — how the 7 crates relate, what each owns, which depend on which
- **Design a new crate** (e.g. `notforge`) — the existing design docs establish the hexagonal port/adapter pattern and crate ownership conventions to follow
- **Understand bootstrap flow** — `notstrap` orchestration order: prereqs → config → clone → age key → decrypt → link → hooks → report
- **Implement or extend hooks** — `nothooks` phase model (dot vs. setup), state persistence, Nushell-only execution
- **Work on secret resolution** — `notsecrets` multi-provider chain, age encryption, identity types
- **Run or extend CI** — `taskit ci` pipeline, step table, gate definitions
- **Understand configuration** — `notfiles.toml` schema, package filtering, per-package overrides
- **Plan integration tests** — cross-crate test patterns in `tests/integration/`, `BootstrapOptions`, `run()` entry point

Do **not** use this skill to look up current code — read the source directly. Use it to understand intent, design rationale, and system structure.

---

## Key Concepts

### Hexagonal Architecture

All crates use ports/adapters. Key pattern:

- **Port** = trait defined in the crate (e.g. `FileStore`, `SecretResolverPort`, `ForgeApiPort`)
- **Adapter** = concrete impl (e.g. `FileStoreImpl`, `InMemoryFileStore`, `OpSource`)
- **Domain** = business logic that depends only on ports, never on adapters directly

This makes every layer independently testable with fakes. The `InMemoryFileStore` in `notfiles` is the canonical example.

### Crate Ownership Model

Each crate has a single responsibility and owns its own error types, config types, and ports:

| Crate | Single responsibility |
|---|---|
| `notcore` | Shared primitives only — errors, config types, `expand_tilde`, `HookPhase`, `Reporter` |
| `notfiles` | Symlink/copy engine — link, unlink, status, diff, adopt |
| `notsecrets` | Secret resolution + age encryption/decryption, no external binaries |
| `nothooks` | Nushell hook runner, phase gating, state persistence |
| `notstrap` | Orchestration only — calls the other crates in sequence |
| `notnet` | Network/Tailscale presence detection |
| `notgraph` | Dependency graph analysis — HTML/MD/JSON/Mermaid output |

Dependency direction: `notstrap` → everything; `notcore` ← everything. `notfiles` does not depend on `notsecrets` or `nothooks`.

### Hook Phases

`nothooks` has two phases:

- **`HookPhase::Dot`** — always runs on every `notstrap` invocation
- **`HookPhase::Setup`** — runs once, state tracked in `.nothooks-state.toml`; skipped on reruns unless `--force`

All hook scripts are Nushell (`.nu`). No `.sh` scripts anywhere in the workspace.

---

## CI Pipeline

From `documentation/api/ci-pipeline.md` (high confidence):

Run the full pipeline:

```sh
taskit ci
```

Steps are gated — a failing step blocks subsequent ones. See `references/documentation/` for the full step table.

Configuration lives in `taskit.toml`. Sections map to pipeline stages.

---

## notforge Design (Approved, Not Yet Implemented)

From `documentation/architecture/2026-07-08-on-demand-gitea-forge-design.md` (high confidence):

**Goal:** New `notforge` crate and CLI — manages Gitea availability on demand, creates repos via API, wires git remotes.

**Crates to create:**

```text
crates/notforge/
  Cargo.toml
  src/
    lib.rs       # public API surface
    config.rs    # forge config, backend mode, repo specs, secret refs
    error.rs     # forge-specific errors
    ports.rs     # ForgeApiPort, LifecyclePort, GitRemotePort, SecretResolverPort
    gitea.rs     # Gitea API request/response types, adapter boundary
    lifecycle.rs # command planning for local vs VM Gitea
    git.rs       # remote URL derivation, git remote ops
    main.rs      # notforge CLI
  tests/
    integration.rs
```

**Dependency rules:**

- `notforge` → `notcore` (optional, for report types)
- `notforge` → `notsecrets` (via port adapter, for credentials)
- `notstrap` → `notforge` (future, deferred until API is stable)
- `notforge` must NOT depend on `notstrap`
- `notnet` may be composed later for tailnet readiness, not replaced

**Risk checklist:**

- All HTTP, git, SSH, and process ops must stay behind ports
- Credentials must not be printed, logged, or stored in config

---

## Workspace Implementation Patterns

From `documentation/other/2026-03-31-notfiles-workspace.md` (high confidence):

### Config Type Pattern

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub defaults: Defaults,
    // per-package overrides in packages: HashMap<String, PackageConfig>
}
```

### Error Type Pattern

```rust
#[derive(Debug, thiserror::Error)]
pub enum NotfilesError {
    #[error("config file error: {0}")]
    Config(String),

    #[error("package not found: {name}")]
    PackageNotFound { name: String },

    #[error("conflict at {path}: {reason}")]
    Conflict { path: PathBuf, reason: String },

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}
```

### Workspace Cargo.toml Pattern

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
serde       = { version = "1", features = ["derive"] }
thiserror   = "2"
toml        = "0.8"
notcore     = { path = "crates/notcore" }
```

---

## Integration Test Patterns

From `documentation/other/2026-04-01-integration-tests.md` (high confidence):

The `tests/integration/` workspace crate imports all feature crates and tests cross-crate boundaries.

**`notstrap` public entry point:**

```rust
// notstrap::run() is the testable entry point
pub struct BootstrapOptions { /* ... */ }
pub fn run(opts: BootstrapOptions) -> anyhow::Result<()> { /* ... */ }
```

**Test structure:**

```rust
// tests/integration/tests/bootstrap.rs
#[test]
fn test_full_bootstrap_dot_hooks_only() { /* uses tempdir + fake dotfiles */ }

#[test]
fn test_setup_hooks_skipped_on_rerun() { /* verifies state persistence */ }

#[test]
fn test_bootstrap_fails_fast_on_bad_key() { /* verifies error propagation */ }
```

Run integration tests: `cargo nextest run -p integration`

---

## Design Principles Applied

From `documentation/architecture/DESIGN.md` (clipping of Rust Design Patterns, high confidence):

The workspace follows these principles explicitly:

- **SRP** — each crate has one responsibility; adding forge logic goes in `notforge`, not `notstrap`
- **OCP** — new secret providers added via `SecretResolver` trait without modifying existing providers
- **Composition over inheritance** — Rust traits for polymorphism, not inheritance chains
- **DRY** — shared types in `notcore`; each crate imports rather than redefines

---

## Available Reference Files

| Path | Contents | Source | Confidence |
|---|---|---|---|
| `references/documentation/architecture/` | Design specs, ADRs, DESIGN.md clipping | documentation | high |
| `references/documentation/api/` | CI pipeline steps, `taskit.toml` configuration | documentation | high |
| `references/documentation/other/` | Workspace implementation plan, integration test plan | documentation | high |
| `references/documentation/specifications/` | Crate specs: notfiles workspace, notgraph, notsecrets age redesign, Tailscale, integration tests | documentation | high |
| `references/config_patterns/config_patterns.md` | Config extraction report — 2 files, 12 settings | unknown | medium |
| `references/dependencies/` | Dependency graph and analysis | analysis | varies |

### Navigation Tips

- Start with `references/documentation/architecture/` for system design context
- For implementing a new crate, read the `notforge` design doc first — it's the most complete recent ADR
- For CI questions, go to `references/documentation/api/ci-pipeline.md`
- For test patterns, read `references/documentation/other/2026-04-01-integration-tests.md`
- Config pattern reference is lower confidence (source classification unknown) — verify against `notfiles.toml` in the repo

---

## Working with This Skill

**Beginners:** Start with the Key Concepts section to understand hexagonal architecture and crate ownership before reading specs.

**Intermediate:** Use the design docs in `references/documentation/specifications/` when implementing changes to existing crates. The specs describe the intended API boundaries.

**Advanced:** The `notforge` design doc is the most complete blueprint for adding a new crate from scratch — use it as a template for future additions. It covers ports, adapters, dependencies, test coverage, and risk checklist.

**Conflict resolution:** This skill has no known discrepancies between sources. The documentation source is high confidence. For any conflict with what you see in actual source code, trust the code — these docs describe intent, not always current state.

---

*Sources: 7 high-confidence documentation files + 1 medium-confidence config analysis | Generated 2026-07-15*
