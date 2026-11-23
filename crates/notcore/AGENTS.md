# notcore

Shared types for the whole workspace. Every other crate depends on this one.

## Dependency boundary — the one hard rule

**`notcore` must not depend on any other workspace crate.** It is the leaf.
`scripts/check-dep-boundaries.py` fails CI if that ever changes. If you find
yourself wanting `notfiles` or `notsecrets` here, the type belongs in the
caller, or the shape needs to be generic (see `load_toml_file`).

## Modules

| Module        | Holds                                                                 |
| ------------- | --------------------------------------------------------------------- |
| `config.rs`   | `Config`, `Defaults`, `PackageConfig`, `Method`, `suggest_package`     |
| `error.rs`    | `NotfilesError` — the shared error enum                               |
| `paths.rs`    | `expand_tilde`, `dotfiles_dir`                                        |
| `reporter.rs` | `Reporter` trait, `LinkEvent`, `SilentReporter`                       |
| `types.rs`    | `HookPhase`, `HookSpec`, `PackageSpec`, `Report`, `Step`, `StepStatus` |

## What lives here vs. downstream

This crate holds *data and traits*, not behavior. `Config` knows how to answer
questions about itself (`method_for`, `target_for`, `ignore_patterns_for`,
`is_package_for_current_platform`), but it never touches the filesystem beyond
`Config::load`. Filesystem work belongs behind `notfiles`' `FileStore` port.

## Gotchas

- `Config::validate()` rejects `include` and `exclude` being set together.
  Callers are expected to call it before acting on a config — most do it right
  after `Config::load`.
- `expand_tilde` takes `&str`, not `&Path`. Callers with a `PathBuf` go through
  `to_string_lossy()`.
- `load_toml_file` is generic over the error type so downstream crates can map
  I/O and parse failures into *their* error enum without notcore knowing about
  it. Reach for it instead of adding a new dependency here.
- `Method` derives `Copy` — pass it by value.

## Testing

```bash
cargo test -p notcore
```

Unit tests are inline (`#[cfg(test)] mod tests`). There is no `tests/` dir —
this crate has no I/O to integration-test.
