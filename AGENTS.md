# AGENTS.md

Guidance for agents working in this repository (the notfiles dotfiles manager).

This file is deliberately a pointer index — details live in [docs/AGENTS-notfiles.md](docs/AGENTS-notfiles.md), which covers: what notfiles is, build/test commands, the workspace structure and crate purposes, the hexagonal architecture (ports/adapters), module keynotes, notsecrets/nothooks/notstrap/notgraph details, and the notfiles.toml configuration contract. Read it with `read` when you need the specifics.

**Quick orientation:** it's a Cargo workspace with 7 crates under `crates/`; the primary user-facing tool is the `notfiles` binary (link/unlink/status/check/diff/adopt/completions). Build and test with plain `cargo` (`cargo build -p notfiles`, `cargo test -p notfiles`); if you act through another harness, check its own guide in `docs/AGENTS-notfiles.md`.
