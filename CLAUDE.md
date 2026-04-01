# CLAUDE.md

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

This is a Cargo workspace with 5 crates under `crates/`:

| Crate | Purpose |
|-------|---------|
| `notcore` | Shared types: `Config`, `NotfilesError`, `expand_tilde`, `HookPhase`, `HookSpec`, `Report`, `StepStatus` |
| `notfiles` | Dotfiles linker — lib + `notfiles` binary (symlink/copy, init, status) |
| `notsecrets` | Age key retrieval via Bitwarden, file, or prompt; SOPS decryption |
| `nothooks` | Nushell hook runner with dot/setup phases and state persistence |
| `notstrap` | New-machine bootstrap orchestrator — ties all crates together |

## Architecture

### notfiles (primary user-facing tool)

Four subcommands: `init`, `link`, `unlink`, `status`. CLI parsing in `crates/notfiles/src/cli.rs`; dispatch in `src/main.rs`.

**Core flow for `link`:** `main` → `resolve_packages` → `collect_files` (recursive walk with ignore filtering) → `linker::link_package` (create symlinks or copies, record in state).

Key modules in `crates/notfiles/src/`:
- **linker** — Creates/removes symlinks or copies. Manages `State` (`.notfiles-state.toml`) tracking every linked file. Handles conflict detection, `--force` backups, empty-parent cleanup on unlink.
- **config** — Parses `notfiles.toml`. Per-package overrides for method, target, ignore. Falls back to `[defaults]`.
- **package** — Discovers packages (non-hidden subdirs) and recursively collects files via `IgnoreMatcher`.
- **ignore** — Glob-based ignore matching using `globset`.
- **status** — Compares expected vs actual state: linked/copied/missing/conflict/orphan.

### notsecrets

`AgeKeySource` trait with three implementations: `BitwardenSource` (bw CLI), `FileSource`, `PromptSource`. `resolve_age_key()` tries each in order. `install_age_key()` writes to `~/.config/sops/age/keys.txt` (mode 0600). `decrypt_sops()` shells out to `sops --decrypt`.

### nothooks

`HookRunner` executes `.nu` scripts via `nu <script>`. Two phases: `HookPhase::Dot` (always runs) and `HookPhase::Setup` (runs once, tracked in `.nothooks-state.toml`). `--force` flag reruns setup hooks.

### notstrap

Orchestrates in order: prereqs check → load config → clone dotfiles → age key → decrypt SOPS → link dotfiles → run hooks → final report. Config file: `notstrap.toml`.

## Configuration

`notfiles.toml` lives at the dotfiles directory root. Each subdirectory is a "package". Per-package config can override: `method` (symlink/copy), `target` directory, additional `ignore` patterns.

## Edition

Rust edition 2024. All hook scripts are Nushell (`.nu`) — no `.sh` scripts.

## CI / Gitea Actions

Workflows live in `.gitea/workflows/` — mirrors `.github/workflows/` for GitHub.
The `public-ready.yml` workflow checks secrets, private IPs, licenses, and tracked secrets files on every push to main.
