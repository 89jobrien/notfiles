# notfiles

The dotfiles linker itself — both a library and the `notfiles` binary
users run day to day.

## Subcommands

`init`, `link`, `unlink`, `status`, `check`, `diff`, `adopt`,
`completions`. Global flags: `--dry-run`, `--verbose`, `--json`.

CLI parsing lives in `src/cli.rs`; dispatch in `src/main.rs`.

## Core flow for `link`

```text
main
  → config.validate()
  → resolve_packages_filtered   (include/exclude + platform filtering)
  → collect_files               (recursive walk with ignore filtering)
  → linker::link_package        (create symlinks/copies, record state,
                                  return LinkResult with counters)
```

## Modules

- **linker** — Creates/removes symlinks or copies via the `FileStore`
  port. Manages `State` (`.notfiles-state.toml`). Returns `LinkResult`
  with linked/copied/skipped/backed_up counts. Also provides
  `adopt_files`.
- **package** — Discovers packages with include/exclude and platform
  filtering. Recursively collects files via `IgnoreMatcher`. Typo
  suggestions via `notcore::suggest_package` on not-found errors.
- **ignore** — Glob-based ignore matching using `globset`.
- **status** — Compares expected vs. actual state:
  linked/copied/missing/conflict/orphan. Also provides `diff_package`
  for copy-method divergence detection.
- **adapters/** — `FileStoreImpl`, `InMemoryFileStore`,
  `TerminalReporter`, `JsonReporter`.
- **ports** — the `FileStore` trait definition.

See [ADR 0002](../adr/0002-hexagonal-architecture.md) for why the
crate is structured this way, and the
[Configuration reference](../reference/configuration.md) for
`notfiles.toml` shape.
