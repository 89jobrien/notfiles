# notgraph

Dependency/import graph generation for the workspace's own Rust
source. Not part of the runtime dotfiles-linking flow — a development
tool for understanding and validating the workspace's own structure.

## Usage

```bash
cargo run -p notgraph
```

Regenerates output under `target/notgraph` (default; override with
`--output`). Useful flags:

- `--fail-on-cycles` — exit non-zero if a dependency cycle is found
  between workspace crates.
- `--top <N>` — number of hottest modules to report in the heatmap
  (default 10).

## What it produces

- Crate graph — built from the workspace `Cargo.toml` manifest via
  `crate_graph::build`.
- Per-crate module graphs and symbol tables, built by walking each
  workspace member's source with `cargo_metadata`.
- Output formats: HTML, Markdown, JSON, and Mermaid diagrams.
- Cycle detection and a module "heatmap" (highest fan-in/fan-out
  modules).

## When to run it

Run `cargo run -p notgraph` after adding or removing crates, or after
significant module reorganization, to regenerate the graph in
`target/notgraph` and confirm no new dependency cycles were
introduced.
