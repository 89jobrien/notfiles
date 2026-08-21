# notfiles

The dotfiles linker — lib + the `notfiles` binary. This is the primary
user-facing crate.

## Dependency boundary

May depend on `notcore` only. **Not** on `notsecrets` or `nothooks` — they are
sibling feature crates and `scripts/check-dep-boundaries.py` fails CI on any
edge between them. `notstrap` is the orchestrator that combines them.

## Architecture: ports and adapters

Nothing in this crate calls `std::fs` directly. Two ports:

- **`FileStore`** (`ports.rs`) — all filesystem I/O.
  Adapters: `FileStoreImpl` (real), `InMemoryFileStore` (tests).
- **`Reporter`** (from notcore) — all output.
  Adapters: `TerminalReporter` (ANSI), `JsonReporter` (NDJSON).

Every public function comes in two forms: a convenience one that wires up the
real adapters, and a `_with_store` variant taking `&dyn FileStore`. Add new
logic to the `_with_store` form so it stays testable against
`InMemoryFileStore`.

## Modules

| Module      | Responsibility                                                        |
| ----------- | --------------------------------------------------------------------- |
| `cli.rs`    | clap definitions only. Dispatch lives in `main.rs`.                   |
| `linker.rs` | Create/remove links and copies. Owns `State` + `.notfiles-state.toml`. |
| `package.rs`| Discover packages, filter by include/exclude/platform, collect files.  |
| `ignore.rs` | Glob ignore matching via `globset`.                                   |
| `status.rs` | Expected vs. actual: linked/copied/missing/conflict/orphan; `diff_package`. |
| `which.rs`  | Reverse lookup from a target path back to its package and source file. |
| `detect.rs` | Sniff existing stow/chezmoi/notfiles repos under `$HOME`.             |
| `adapters/` | `FileStoreImpl`, `InMemoryFileStore`, `TerminalReporter`, `JsonReporter`. |

## State

`.notfiles-state.toml` at the dotfiles root records every placed file
(`package`, `source`, `target`, `method`, `linked_at`). It is written
atomically — temp file then rename — so a crash mid-write can't truncate it.

State is a record of what *was* done, not the source of truth for what *should*
exist; that comes from `notfiles.toml` plus the package dirs. Code that answers
"where did this come from?" checks state first and falls back to the
filesystem, because state can be stale or absent (`status.rs`, `which.rs` both
do this).

## Adding a subcommand

1. Variant in `cli.rs::Command` with a doc comment (clap shows it as help).
2. Logic in its own module, `_with_store`-style, taking `&dyn FileStore`.
3. Arm in `main.rs` that builds the adapters and honors `--json`.
4. Unit tests against `InMemoryFileStore`; integration tests in
   `tests/integration.rs` that drive the built binary.
5. Update the subcommand lists in the root `README.md`, `CLAUDE.md`, and
   `AGENTS.md` — they enumerate every subcommand and go stale easily.

Feature stubs for planned subcommands are parked as `// TODO(name):` comments
at the top of `cli.rs`. Delete the TODO in the same commit that implements it.

## Gotchas

- Global flags (`--dry-run`, `--verbose`, `--json`, `--dir`) are declared
  `global = true`, so they can appear before *or* after the subcommand.
- `--dry-run` must not write state. Check the flag before `state.save`.
- Path comparison against state entries is string equality on absolute paths.
  Normalize user input first — tilde-expand, make absolute, strip `.`/`..` —
  or `./.gitconfig` silently fails to match. `which::normalize_target` does this.
- `unlink` deliberately tolerates a missing package dir, since state entries can
  outlive the directory.

## Testing

```bash
cargo test -p notfiles                  # unit + integration
cargo test -p notfiles --lib            # unit only
```

`tests/integration.rs` shells out to the built binary and asserts on stdout and
exit status; `tests/fixtures/` holds a sample dotfiles tree used by the
`test_fixture_*` cases.
