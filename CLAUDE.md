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
|              | `unlink`, `status`, `check`, `diff`, `adopt`, `which`, `doctor`,           |
| `notsecrets` | Multi-provider secret resolution (`SecretResolver`), age encryption/       |
|              | decryption, identity management                                            |
| `nothooks`   | Nushell hook runner with dot/setup phases and state persistence            |
| `notnet`     | Tailnet presence — `ensure_connected`, auth key via env/YubiKey PIV/prompt |
| `notstrap`   | New-machine bootstrap orchestrator — ties all crates together              |
| `notgraph`   | Dependency/import graph for Rust files — HTML/MD/JSON/Mermaid output,      |
|              | heatmap, cycle detection                                                   |
| `nottools`   | Tool reconciliation: declared (mise, Brewfile) vs installed, alias targets |
|              | and undeclared `cargo install` crates; backs `notfiles doctor`             |
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

Nine subcommands: `init`, `link`, `unlink`, `status`, `check`, `diff`,
`adopt`, `which`, `doctor`, `completions`. Global flags: `--dry-run`,
`--verbose`, `--json`.
CLI parsing in `crates/notfiles/src/cli.rs`; dispatch in `src/main.rs`.

Per-crate detail lives in each crate's own `CLAUDE.md`:

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
- **which** — Reverse lookup from a target path to its source package and
  file. Tries recorded state entries, then `read_link`, then the packages
  configured in `notfiles.toml`.
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

Orchestrates in order: load config → prereqs check → clone dotfiles → age key → decrypt SOPS → link dotfiles → run hooks → reconcile tools → final report. Config file: `notstrap.toml`.

The prereq list comes from an optional `[prereqs] required = [...]` table (default `nu`, `sops`, `age`); it runs before the clone, so it cannot read the dotfiles manifest. The final `tools` step runs `nottools` against the cloned `notfiles.toml` `[doctor]` table: it installs declared-but-missing tools (`mise install`, `brew install`) and reports alias targets declared nowhere. It fails the bootstrap only when a declared tool is still unresolved, and is skipped when there is no `[doctor]` table.

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

`notfiles.toml` lives at the dotfiles directory root.

**Package contract:** declare packages explicitly with `packages = [...]` under
`[defaults]`. Only those top-level directories are packages, so operational
directories like `scripts/` or `docs/` are never linked. Every entry must be a
single directory name (no `/`, `..`) that exists as a directory; a typo or a
non-directory is an error (with a `did you mean` suggestion). When `packages` is
absent, notfiles falls back to the deprecated auto-discovery of every non-hidden
top-level directory and prints a deprecation warning once per run (stderr).
`exclude` still applies on top of either mode.

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

### nottools

Hexagonal like `notfiles`: ports (`Fs`, `ToolProbe`, `Installer`) in `ports.rs`, system and in-memory
adapters in `adapters.rs`, pure domain in `aliases`, `declared`, `doctor` and `fix`. Configured by a
`[doctor]` table in `notfiles.toml` (`mise`, `brewfiles`, `alias_files`). `notfiles doctor` reports alias
targets that do not resolve (with the install command when declared in mise or a Brewfile) and
`cargo install` crates no manifest declares. `--fix` runs the installs; `--adopt` declares
crates.io crates in the mise config; both honor `--dry-run`. Aliases inside an
`if command -v <target>; then` guard are treated as optional and skipped.

#### notstrap wizard and `--yes`

On a terminal, `notstrap run` first runs a wizard (`wizard.rs`, over the `Prompter` port in `prompt.rs`):
it asks for the clone URL, the dotfiles directory, whether to join the tailnet (when `[tailscale]` is
configured) and which hooks to run, prints the plan, and asks to proceed. Answers overlay the loaded
config; declining cancels before anything changes. `--yes` / `-y` skips the wizard, accepts every
default, and exports `NOTSTRAP_YES=1` to hooks so they skip their own prompts. Without a terminal
there are no prompts either, so scripted runs behave as before. nothooks also exports `NOTFILES_DIR`
(the dotfiles checkout) to every hook and runs `.rs` hooks with `rust-script`.

## Testing layers

- **Conformance**: one contract per port, run against every adapter and its double
  (`notfiles/tests/conformance_file_store.rs`, `nottools/tests/conformance_ports.rs`,
  `nothooks/tests/conformance_ports.rs`, `notnet/tests/conformance_command_runner.rs`,
  `notstrap/tests/conformance_git_clone.rs`, `notsecrets/tests/conformance_secret_source.rs`).
  A new port or adapter gets a contract test; a double that is looser than the real
  adapter is a bug in the double.
- **Property** (`proptest`): `notfiles` (`detect`, ignore matcher), `notsecrets` (age header),
  `nothooks` (state store). Commit `proptest-regressions/`; never delete counterexamples.
- **Fuzz** (`fuzz/`, excluded from the workspace): age header parsing and decryption. Needs
  nightly and `cargo-fuzz`: `cargo +nightly fuzz run fuzz_parse_header -- -max_total_time=60`.
  Seed corpus in `fuzz/corpus/` is committed; regenerate with
  `cargo run --manifest-path fuzz/Cargo.toml --bin gen_corpus`.

## Policy notes

- No Python anywhere in the repo or its CI. Glue is `rust-script` (`scripts/*.rs`) and checks are
  `cargo test`: crate dependency boundaries live in `tests/integration/tests/dep_boundaries.rs`,
  and the release workflow computes affected crates with `scripts/affected-crates.rs`.
- notstrap never mutates its own process environment. Secrets resolved from `notsecrets.toml`
  are passed to hooks (and only hooks) as environment variables.
