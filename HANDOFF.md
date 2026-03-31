# HANDOFF.md

State of the `notfiles` repo as of 2026-03-31.

## What Was Done

Restructured from a flat `src/` layout into a Cargo workspace of 5 crates:
- `notcore` — shared library (types, config, paths, errors)
- `notfiles` — dotfiles linker, migrated from `src/`
- `notsecrets` — age key retrieval (Bitwarden/file/prompt) + SOPS decrypt
- `nothooks` — Nushell hook runner with phase/state tracking
- `notstrap` — new-machine bootstrap orchestrator

39 tests passing, 0 clippy warnings. Committed to `main`, not yet pushed to `gitea/main`.

## Pending Issues

### 1. Push to remote
```bash
git push
```
8 commits ahead of `gitea/main`. Nothing blocking this.

### 2. notstrap has an empty lib.rs
`crates/notstrap/src/lib.rs` is empty — created as a stub. `notstrap` is binary-only so this is harmless, but it's dead weight. Either delete `[lib]` from `notstrap/Cargo.toml` (if there is one) or remove the file.

### 3. notstrap config not documented
`notstrap.toml` format is defined inline in `notstrap/src/main.rs` via serde structs but never written down. A sample `notstrap.toml` should be created and documented.

### 4. notsecrets has no binary
`notsecrets` is library-only. A small `notsecrets get-key` CLI binary could be useful for manual debugging/testing of the key retrieval chain, but not blocking.

### 5. Integration test coverage for nothooks assumes nu in PATH
`nothooks` integration tests call `nu` directly. If `nu` is not on PATH (e.g. on CI), the success/failure tests will fail for the wrong reason. CI should either install `nu` or the tests should be marked `#[ignore]` with a feature flag.

### 6. notstrap sops_file path is always joined to dotfiles_dir
If `sops_file` in `notstrap.toml` is an absolute path, the join will silently override the base. This is a minor footgun in `repo.clone_if_missing` usage — low priority.
