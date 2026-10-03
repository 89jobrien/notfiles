# Architecture Overview

`notfiles` is a Cargo workspace (edition 2024) of 8 crates under
`crates/`, plus a cross-crate integration test crate under
`tests/integration/`.

| Crate        | Purpose                                                                 |
| ------------ | ------------------------------------------------------------------------ |
| `notcore`    | Shared types: `Config`, `NotfilesError`, `Reporter`, `LinkEvent`, `expand_tilde`, `suggest_package`, `HookPhase`, `HookSpec`, `Report` |
| `notfiles`   | Dotfiles linker — lib + `notfiles` binary. Subcommands: `init`, `link`, `unlink`, `status`, `check`, `diff`, `adopt`, `completions` |
| `notsecrets` | Multi-provider secret resolution (`SecretResolver`), native age encryption/decryption, identity management |
| `nothooks`   | Nushell hook runner with dot/setup phases and state persistence         |
| `notnet`     | Network utilities — Tailscale integration, YubikeySource                 |
| `notstrap`   | New-machine bootstrap orchestrator — ties all crates together           |
| `notgraph`   | Dependency/import graph for Rust files — HTML/MD/JSON/Mermaid output, heatmap, cycle detection |
| `notforge`   | Gitea API integration (repo/PR/issue management)                        |

## Dependency direction

`notcore` sits at the bottom of the graph and is depended on by every
other crate — it owns the shared `Config`, error, and reporting types
so no two crates invent their own copies. `notstrap` sits at the top:
it is the only crate that pulls in `notfiles`, `notsecrets`, `nothooks`,
and `notnet` together, because bootstrapping a new machine is the one
workflow that needs all of them at once. `notgraph` and `notforge` are
siblings, not dependents — `notgraph` analyzes the workspace's own
source tree, and `notforge` talks to a Gitea instance; neither is a
build-time or runtime dependency of the others.

Run `cargo run -p notgraph` to regenerate an up-to-date crate/module
graph, cycle report, and heatmap under `target/notgraph`.

## Architecture pattern: ports and adapters

`notfiles` (the linker crate) follows a hexagonal architecture — see
[ADR 0002](../adr/0002-hexagonal-architecture.md). Two ports,
`FileStore` and `Reporter`, decouple business logic (`linker`,
`package`, `status`, `ignore`) from I/O:

- `FileStoreImpl` / `InMemoryFileStore` — real filesystem vs. in-memory
  fake, letting linker/status logic be unit-tested without touching
  disk.
- `TerminalReporter` / `JsonReporter` — human-readable ANSI output vs.
  NDJSON, selected by the global `--json` flag.

`notforge` and `notnet` follow the same pattern for their respective
external integrations (`ports.rs` in each crate defines the trait
boundary; concrete adapters implement it).

## Configuration and state

- `notfiles.toml` lives at the dotfiles directory root and declares
  packages, include/exclude filtering, per-package `method`/`target`/
  `ignore`/`platforms` overrides.
- `.notfiles-state.toml` (managed by `notfiles`) and
  `.nothooks-state.toml` (managed by `nothooks`) track what has been
  linked and which setup hooks have already run, so re-running `link`
  or bootstrap is idempotent.
- `notstrap.toml` configures the bootstrap orchestrator: prereqs check
  → load config → clone dotfiles → age key → decrypt SOPS → link
  dotfiles → run hooks → final report.

## Secrets

See [notsecrets](../crates/notsecrets.md) and
[ADR 0003](../adr/0003-native-age-encryption.md) /
[ADR 0005](../adr/0005-multi-provider-secret-resolution.md) for how
secret resolution and encryption are structured.
