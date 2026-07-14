# CLAUDE.md

<preflight>
  Read: 
    - @docs/DESIGN.md
    - @.ctx/HANDOFF.md
</preflight>

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is notfiles?

A modern dotfiles manager written in Rust — a pure Rust alternative to GNU Stow. It symlinks (or copies) files from organized "package" directories into a target location (typically `~`).

## Build & Test Commands

```bash
cargo build                        # build all workspace crates
cargo build -p notfiles            # build just the notfiles binary
cargo test                         # run all tests across workspace
cargo test -p notcore              # test notcore only
cargo test -p notfiles             # test notfiles only
cargo test -p notsecrets           # test notsecrets only
cargo test -p nothooks             # test nothooks only
cargo clippy --workspace           # lint all crates
cargo fmt --check                  # check formatting
```

## Workspace Structure

This is a Cargo workspace with 7 crates under `crates/`:

| Crate        | Purpose                                                                 |
| ------------ | ----------------------------------------------------------------------- |
| `notcore`    | Shared types: `Config`, `NotfilesError`, `Reporter`, `LinkEvent`,       |
|              | `expand_tilde`, `suggest_package`, `HookPhase`, `HookSpec`, `Report`    |
| `notfiles`   | Dotfiles linker — lib + `notfiles` binary. Subcommands: `init`, `link`, |
|              | `unlink`, `status`, `check`, `diff`, `adopt`, `completions`             |
| `notsecrets` | Multi-provider secret resolution (`SecretResolver`), age encryption/    |
|              | decryption, identity management                                         |
| `nothooks`   | Nushell hook runner with dot/setup phases and state persistence         |
| `notnet`     | Network utilities — Tailscale integration, YubikeySource                |
| `notstrap`   | New-machine bootstrap orchestrator — ties all crates together           |
| `notgraph`   | Dependency/import graph for Rust files — HTML/MD/JSON/Mermaid output,   |
|              | heatmap, cycle detection                                                |

## Architecture

### notfiles (primary user-facing tool)

Eight subcommands: `init`, `link`, `unlink`, `status`, `check`, `diff`,
`adopt`, `completions`. Global flags: `--dry-run`, `--verbose`, `--json`.
CLI parsing in `crates/notfiles/src/cli.rs`; dispatch in `src/main.rs`.

**Core flow for `link`:** `main` → `config.validate()` →
`resolve_packages_filtered` (include/exclude + platform filtering) →
`collect_files` (recursive walk with ignore filtering) →
`linker::link_package` (create symlinks or copies, record in state,
return `LinkResult` with counters).

**Architecture**: Hexagonal (ports/adapters). `FileStore` trait abstracts
filesystem I/O; `Reporter` trait abstracts output. Adapters:
`FileStoreImpl` (real fs), `InMemoryFileStore` (testing),
`TerminalReporter` (ANSI), `JsonReporter` (NDJSON).

Key modules in `crates/notfiles/src/`:

- **linker** — Creates/removes symlinks or copies via `FileStore` port.
  Manages `State` (`.notfiles-state.toml`). Returns `LinkResult` with
  linked/copied/skipped/backed_up counts. Also provides `adopt_files`.
- **package** — Discovers packages with include/exclude and platform
  filtering. Recursively collects files via `IgnoreMatcher`. Typo
  suggestions via `suggest_package` on not-found errors.
- **ignore** — Glob-based ignore matching using `globset`.
- **status** — Compares expected vs actual state:
  linked/copied/missing/conflict/orphan. Also provides `diff_package`
  for copy-method divergence detection.
- **adapters/** — `FileStoreImpl`, `InMemoryFileStore`,
  `TerminalReporter`, `JsonReporter`.
- **ports** — `FileStore` trait definition.

### notsecrets

Multi-provider secret resolution via `SecretResolver`. Providers include
`EnvSource`, `OpSource` (1Password), `BitwardenSource`, `FileSource`,
`DotenvxSource`, `SopsSource`, and others. Native age encryption/decryption
with x25519, SSH ed25519/RSA, and scrypt identity support. No external
`sops` or `age` binaries required.

### nothooks

`HookRunner` executes `.nu` scripts via `nu <script>`. Two phases: `HookPhase::Dot` (always runs) and `HookPhase::Setup` (runs once, tracked in `.nothooks-state.toml`). `--force` flag reruns setup hooks.

### notstrap

Orchestrates in order: prereqs check → load config → clone dotfiles → age key → decrypt SOPS → link dotfiles → run hooks → final report. Config file: `notstrap.toml`.

## Configuration

`notfiles.toml` lives at the dotfiles directory root. Each subdirectory
is a "package".

Global defaults support `include` (allowlist) or `exclude` (blocklist)
for package filtering — mutually exclusive. Per-package config can
override: `method` (symlink/copy), `target` directory, additional
`ignore` patterns, and `platforms` (e.g. `["linux"]`, `["macos"]`) to
gate packages to specific OSes.

## Edition

Rust edition 2024. All hook scripts are Nushell (`.nu`) — no `.sh` scripts.

## CI / Gitea Actions

Workflows live in `.gitea/workflows/` — mirrors `.github/workflows/` for GitHub.
The `public-ready.yml` workflow checks secrets, private IPs, licenses, and tracked secrets files on every push to main.
