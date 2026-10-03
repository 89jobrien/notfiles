# integration

`tests/integration` — a separate Cargo package holding cross-crate
integration tests. These exist outside any single crate's `tests/`
directory because they exercise combinations of crates rather than one
crate in isolation, and shouldn't force every crate to depend on every
other crate's test fixtures.

## What's covered

- **`cross_crate.rs`** — verifies crates that should remain decoupled
  actually are: e.g. `notsecrets` (resolving an age identity via
  `FileSource`) and `nothooks` (running a dot-phase hook) can each be
  used independently within the same test without one requiring the
  other.
- **`bootstrap.rs`** — end-to-end bootstrap: builds a temp dotfiles
  repo with an age key and a `notfiles.toml` package, then drives
  `notstrap::run` against a temp `$HOME` and asserts the full
  prereqs → secrets → link → hooks flow completes.

## Running

```bash
cargo test -p integration
```

(or simply `cargo test`, which runs it as part of the full workspace
test suite).
