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

This is a Cargo workspace with 8 crates under `crates/`.

**Each crate has its own `CLAUDE.md`** with the detail you need when editing
inside it — invariants, gotchas, and scoped test commands. This file stays a
map plus the rules that cross crate boundaries. When working in a crate, read
`crates/<crate>/CLAUDE.md` first.

| Crate        | Purpose                                                                    |
| ------------ | -------------------------------------------------------------------------- |
| `notcore`    | Shared types: `Config`, `NotfilesError`, `Reporter`, `LinkEvent`,          |
|              | `expand_tilde`, `suggest_package`, `HookPhase`, `HookSpec`, `Report`       |
| `notfiles`   | Dotfiles linker — lib + `notfiles` binary. Subcommands: `init`, `link`,    |
|              | `unlink`, `status`, `check`, `diff`, `adopt`, `which`, `detect`,          |
|              | `completions`                                                             |
| `notsecrets` | Multi-provider secret resolution (`SecretResolver`), age encryption/       |
|              | decryption, identity management                                           |
| `nothooks`   | Nushell hook runner with dot/setup phases and state persistence            |
| `notnet`     | Tailnet presence — `ensure_connected`, auth key via env/YubiKey PIV/prompt |
| `notstrap`   | New-machine bootstrap orchestrator — ties all crates together              |
| `notgraph`   | Dependency/import graph for Rust files — HTML/MD/JSON/Mermaid output,      |
|              | heatmap, cycle detection                                                   |
| `notforge`   | Gitea lifecycle + repo provisioning. Scaffolded: ports and config are      |
|              | defined, the binary is a stub and most impls are unwritten                 |

### Crate dependency boundaries

Enforced by `scripts/check-dep-boundaries.py` — CI fails on a violation:

- **`notcore` is a leaf.** No dependencies on other workspace crates, ever.
- **`notfiles`, `notsecrets`, `nothooks` may depend on `notcore` only** — never
  on each other. If one seems to need another, the composition belongs in
  `notstrap`.
- **`notstrap` is the orchestrator** and may depend on all of them.

`notnet`, `notgraph`, and `notforge` are outside the script's crate set, so
their edges are *not* CI-enforced. Hold them to the same rule by hand.

## Architecture

`notfiles` is the primary user-facing tool: subcommands `init`, `link`,
`unlink`, `status`, `check`, `diff`, `adopt`, `which`, `detect`,
`completions`, with global `--dry-run`, `--verbose`, `--json`, `--dir`.

The workspace is hexagonal throughout — behavior sits behind traits and the
real implementations are injected at the edges. `FileStore` abstracts
filesystem I/O, `Reporter` abstracts output, and `notforge` and `notsecrets`
each define their own ports. In practice that means **most functions have a
`_with_store`-style variant taking the port**, and tests drive those against
in-memory fakes rather than a temp dir.

Per-crate detail lives in each crate's own `CLAUDE.md`:

| Read this                     | When you're working on                       |
| ----------------------------- | -------------------------------------------- |
| `crates/notcore/CLAUDE.md`    | Shared types, config, the leaf-crate rule    |
| `crates/notfiles/CLAUDE.md`   | Linking, state, adding a subcommand          |
| `crates/notsecrets/CLAUDE.md` | Provider chain contract, the age implementation |
| `crates/nothooks/CLAUDE.md`   | Hook phases and setup-hook state             |
| `crates/notnet/CLAUDE.md`     | Tailnet join, auth key chain                 |
| `crates/notstrap/CLAUDE.md`   | Bootstrap order and the report failure model |
| `crates/notgraph/CLAUDE.md`   | Graph analysis and emitters                  |
| `crates/notforge/CLAUDE.md`   | Gitea — read the status note before planning |

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

## Nushell is a test dependency

`nothooks` shells out to `nu`, so `cargo test --workspace` fails in
`tests/integration` (`bootstrap.rs`, `cross_crate.rs`) on any machine without
Nushell on `PATH`. CI installs it. If every hook-related test fails at once,
run `which nu` before debugging the runner.

## Keeping these files in sync

Every `CLAUDE.md` has an `AGENTS.md` twin with identical content, at the repo
root and in each crate. The only difference is the root file's opening line,
which names the assistant. There is no generator — edit both, or the two drift.

## CI / Gitea Actions

Workflows live in `.gitea/workflows/` — mirrors `.github/workflows/` for GitHub.
The `public-ready.yml` workflow checks secrets, private IPs, licenses, and tracked secrets files on every push to main.
