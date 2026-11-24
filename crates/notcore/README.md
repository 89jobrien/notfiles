# notcore

Shared types and configuration for the notfiles dotfiles manager.

This crate is the workspace foundation: `Config` and its `Defaults`,
`PackageConfig`, `Method`, and validation; shared error and path utilities;
and the output abstraction (`Reporter`, `LinkEvent`, `SilentReporter`) that
library code calls instead of `println!`.

## Modules

- `config` — `Config` (parsed from `notfiles.toml` in a dotfiles dir; a
  missing file yields defaults), `Defaults` (global `target`, `ignore`
  list, `include`/`exclude` package filters — mutually exclusive — plus
  platform gating via `Config::is_package_for_current_platform`),
  `PackageConfig` (per-package `method`, `target`, `ignore`, `platforms`),
  `Method` (`symlink` / `copy`), `Config::load` / `Config::validate`,
  `suggest_package` (typo suggestions, Damerau–Levenshtein ≤ 2),
  `load_toml_file`, `starter_toml`.
- `types` — `HookPhase` (`dot`/`setup`), `HookSpec` (name, script, phase,
  interpreter), `PackageSpec`, `Step`, `StepStatus` (`Ok`/`Skipped`/
  `Failed`), `Report` (ordered steps; `has_failures`, `print`).
- `error` — `NotfilesError` (config, package-not-found, conflict, path,
  state, glob, validation, io, other).
- `paths` — `expand_tilde` (`~`, `~/...`), `dotfiles_dir` (`~/dotfiles`).
- `reporter` — `Reporter` trait, `LinkEvent` (Link, Copy, Skip, Backup,
  Remove, CreateDir, DryRun), `SilentReporter`.

## Usage

```rust
use notcore::Config;

let config = Config::load(&dotfiles_dir)?;
config.validate()?;
```

## Testing

`cargo test -p notcore`
