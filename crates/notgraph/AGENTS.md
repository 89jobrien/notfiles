# notgraph

Static analysis tool: builds the crate/module/symbol graph for this workspace
and emits reports. A developer utility — nothing in the dotfiles path depends
on it.

## Pipeline

```
cargo_metadata ──► crate_graph::build   ──┐
                                          ├──► analysis::analyse ──► emit::write_all
syn (parsed .rs) ─► module_graph::build ──┤
                   symbols::build       ──┘
```

- `crate_graph.rs` — workspace crates and their edges, via `cargo_metadata`.
- `module_graph.rs` — per-crate module tree and `use` edges, via `syn`.
- `symbols.rs` — `SymbolTable` of items and their kinds.
- `analysis.rs` — `fan_stats` (fan-in/fan-out), `detect_cycles`, `hotspots`.
- `emit.rs` — writes HTML, Markdown, JSON, and Mermaid output.
- `types.rs` — the shared data types plus `path_to_mod`.

## CLI

```bash
cargo run -p notgraph -- --output docs/graph --top 10 [--fail-on-cycles]
```

`--fail-on-cycles` makes it a CI gate: exits non-zero when an import cycle
exists. Defaults are `--output docs/graph` and `--top 10`.

## Gotchas

- Parsing is `syn`-based, so it sees source text, not a resolved type graph.
  `#[cfg]`-gated modules are read regardless of feature flags, and macro-
  generated items are invisible. Treat the output as a good map, not ground
  truth.
- Output files are generated. Regenerate them with the tool rather than hand-
  editing anything under the `--output` dir.
- `notgraph` is outside `scripts/check-dep-boundaries.py`'s crate set, so its
  deps aren't CI-enforced. It should stay a leaf that no other crate imports.

## Testing

```bash
cargo test -p notgraph
```

`tests/integration.rs` runs against the trees in `tests/fixtures/`.
