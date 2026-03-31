# notfiles Workspace Architecture Design

**Date:** 2026-03-31
**Status:** Approved

## Overview

`notfiles` is a pure-Rust dotfiles manager replacing GNU Stow. This design expands it into a **Cargo workspace** of focused crates that together replace the entire `dotfiles/` shell script ecosystem — including bootstrapping a new machine from scratch.

The primary pain points being solved:

- New machine setup requires too many manual steps before the bootstrap script even runs
- Secrets/1Password setup is fragile — keys aren't available early enough
- Too many shell scripts that are hard to maintain and test
- Nix/Homebrew split is unclear; Rust tools are preferred

---

## Workspace Structure

```
notfiles/                          # Cargo workspace root
├── Cargo.toml                     # [workspace] members
├── notfiles.toml                  # dotfiles symlink config
├── notstrap.toml                  # bootstrap hook/phase config
├── crates/
│   ├── notcore/                   # shared types, config, paths, errors
│   ├── notfiles/                  # symlink engine (stow replacement)
│   ├── notsecrets/                # age key retrieval + sops decrypt
│   ├── nothooks/                  # hook execution engine
│   └── notstrap/                  # new-machine bootstrap orchestrator
└── tests/                         # workspace-level integration tests
```

### Crate Summary

| Crate | Type | Role |
|-------|------|------|
| `notcore` | lib | Shared types, config, paths, errors |
| `notfiles` | lib + bin | Symlink engine (stow replacement) |
| `notsecrets` | lib | Age key retrieval → sops decrypt |
| `nothooks` | lib + bin | Hook execution, phase tracking |
| `notstrap` | bin | Orchestrates everything on a new machine |

---

## Crate Designs

### `notcore` — Shared Foundation

Pure library. No binary. Every other crate depends on it; it depends on nothing in the workspace.

**Contents:**
- `NotfilesError` — unified error enum via `thiserror`
- `Config` — parses `notfiles.toml` + `notstrap.toml`
- `Paths` — `expand_tilde`, `dotfiles_dir()`, standard path resolution
- `HookSpec` — name, script path, phase (`dot` | `setup`)
- `PackageSpec` — name, link method (`symlink` | `copy`), target dir
- `Report` / `Step` — shared types for the end-of-run summary table

**Migration from current code:** `config.rs`, `paths.rs`, `error.rs` move from the current `notfiles/src/` into `notcore`.

---

### `notfiles` — Symlink Engine

Library + binary. The stow replacement. Operates on a dotfiles directory of "packages" (subdirectories) and symlinks their contents into a target (typically `$HOME`).

**Responsibilities:**
- `link` — create symlinks or copies from package dirs to target
- `unlink` — remove symlinks, clean up empty parent dirs
- `status` — diff expected state (config + disk) vs actual state (symlinks + state file)
- `State` — serialize/deserialize `.notfiles-state.toml` (tracks every linked file)
- Conflict detection and `--force` backup handling
- Glob-based ignore matching via `globset`

**Migration from current code:** `linker.rs`, `package.rs`, `ignore.rs`, `status.rs` stay in `notfiles`. Config/paths/error types are imported from `notcore`.

Used as a library by `notstrap` (`notfiles::link()` called directly).

---

### `notsecrets` — Secrets Bootstrap

Library. Retrieves the age private key and decrypts `secrets.sops.env` before anything else runs.

**Age key sources (tried in order):**

1. **Bitwarden CLI** (`bw`) — non-interactive if session is cached; prompts for master password if not
2. **File** — `--key-file <path>` (USB drive, external storage)
3. **Interactive prompt** — user pastes the age key directly

Implemented as a port: `AgeKeySource` trait with three implementations (`BitwardenSource`, `FileSource`, `PromptSource`). `notstrap` iterates sources until one succeeds.

Once the age key is in hand: writes it to `~/.config/sops/age/keys.txt`, then runs `sops --decrypt secrets.sops.env` to produce a live env file. The decrypted env contains all critical credentials: op, bw, github, openai, anthropic, etc.

1Password (`op`) is NOT used at this stage — it's installed later as a hook, after secrets are already available.

---

### `nothooks` — Hook Execution Engine

Library + binary. Replaces `run_hook`, `run_dot_hooks`, `run_setup_hooks` from `bootstrap.sh`.

**Hook phases:**
- `dot` — fast, re-runnable (shell config, git config, AI tool configs)
- `setup` — slow, run-once (package installs, language runtimes, dev tools)

**Responsibilities:**
- Read hook specs from `notstrap.toml` — ordered list with name, script path, phase
- Execute hooks in declared order, capturing stdout/stderr
- Track which `setup` hooks have already run in `.nothooks-state.toml` — skip on re-run unless `--force`
- Feed per-hook pass/skip/fail results into `Report` from `notcore`

**Does NOT handle:** package management (invoked as hooks), secret handling (done before hooks run).

CLI: `nothooks run --phase dot` or `nothooks run --phase setup`

---

### `notstrap` — New Machine Orchestrator

Binary only. The single entry point on a fresh machine.

**Install:**
```bash
cargo install notstrap
notstrap run
```

**Execution order:**

1. Check prerequisites (`op`, `bw`, `sops`, `age` — print exactly what's missing and stop)
2. Run `notsecrets` — retrieve age key → decrypt `secrets.sops.env` → inject into env
3. Clone dotfiles repo if not present (URL from `notstrap.toml`)
4. Run `notfiles link` — stow all packages
5. Run `nothooks run --phase dot` — fast hooks
6. Run `nothooks run --phase setup` — slow hooks (includes `op` install + sign-in)
7. Print `Report` summary table

---

## Day-One Flow

```
cargo install notstrap
notstrap run
  └─ notsecrets
       ├─ try: bw unlock → age key
       ├─ fallback: --key-file <path>
       └─ fallback: interactive prompt
           └─ sops decrypt secrets.sops.env → env injected
  └─ notfiles link (all packages)
  └─ nothooks --phase dot
       └─ shell config, git config, AI tool configs (~seconds)
  └─ nothooks --phase setup
       └─ Homebrew/Nix packages, mise runtimes, dev tools, op install (~minutes)
  └─ Report: ✓ linked 142 files, ✓ 3 dot hooks, ✓ 7 setup hooks
```

---

## Migration Strategy

Migration from `dotfiles/` is gradual — shell scripts die as their Rust equivalents ship:

| Phase | What ships | What it replaces |
|-------|-----------|-----------------|
| 1 | `notfiles` workspace (this design) | GNU Stow |
| 2 | `notsecrets` + `notstrap` skeleton | `setup-secrets.sh`, `install.sh` |
| 3 | `nothooks` | `bootstrap.sh` hook runner |
| 4 | Hook-by-hook Rust rewrites | Individual `setup-*.sh` scripts |
| 5 | `dotfiles/` repo archived | — |

Existing `.rs` scripts (`drift-check.rs`, `redact-audit.rs`, `claude-sessions.rs`, etc.) can migrate into `nothooks` hook scripts or dedicated crates as needed.

---

## Dependencies

| Crate | Key dependencies |
|-------|-----------------|
| `notcore` | `serde`, `toml`, `thiserror`, `dirs`, `anyhow` |
| `notfiles` | `notcore`, `clap`, `globset`, `chrono` |
| `notsecrets` | `notcore`, `clap`, `rpassword`, `which` |
| `nothooks` | `notcore`, `clap`, `serde`, `toml` |
| `notstrap` | `notcore`, `notfiles`, `notsecrets`, `nothooks`, `clap` |

---

## Open Questions

- Should `notstrap` be published to crates.io, or installed from the dotfiles repo directly via `cargo install --path`?
- Should `notsecrets` support a fourth source: a second Bitwarden-compatible backend (Vaultwarden self-hosted)?
- Hook scripts: keep as shell scripts invoked by `nothooks`, or migrate each to a Rust binary over time?
