# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is notfiles?

A modern dotfiles manager written in Rust — a pure Rust alternative to GNU Stow. It symlinks (or copies) files from organized "package" directories into a target location (typically `~`).

## Build & Test Commands

```bash
cargo build                    # build
cargo test                     # run all tests (unit + integration)
cargo test --lib               # unit tests only
cargo test --test integration  # integration tests only
cargo test <test_name>         # run a single test by name
cargo clippy                   # lint
cargo fmt --check              # check formatting
```

## Architecture

The binary has four subcommands: `init`, `link`, `unlink`, `status`. CLI parsing uses clap derive in `src/cli.rs`; command dispatch happens in `src/main.rs`.

**Core flow for `link`:** `main` → `resolve_packages` (discover subdirs or validate requested names) → `collect_files` (recursive walk with ignore filtering) → `linker::link_package` (create symlinks or copies, record in state).

Key modules:
- **linker** — Creates/removes symlinks or copies. Manages `State` (serialized to `.notfiles-state.toml` in the dotfiles dir) which tracks every linked file with source, target, method, and timestamp. Handles conflict detection, `--force` backups, and empty-parent cleanup on unlink.
- **config** — Parses `notfiles.toml`. Provides per-package overrides for method (symlink/copy), target directory, and ignore patterns. Falls back to `[defaults]` section values.
- **package** — Discovers packages (non-hidden subdirectories of the dotfiles dir) and recursively collects files, filtering through `IgnoreMatcher`.
- **ignore** — Glob-based ignore matching using `globset`. Matches both full relative paths and individual path components.
- **paths** — `expand_tilde` utility.
- **status** — Compares expected state (files on disk + config) against actual state (symlinks/copies + state file) to report linked/copied/missing/conflict/orphan.
- **error** — `NotfilesError` enum via `thiserror`.

Integration tests (`tests/integration.rs`) run the compiled binary against temp directories using `tempfile`.

## Configuration

The config file is `notfiles.toml` at the dotfiles directory root. Each subdirectory of the dotfiles dir is a "package". Per-package config can override the link method (`symlink` or `copy`), target directory, and additional ignore patterns.

## Edition

Uses Rust edition 2024.
