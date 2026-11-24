# notgraph

Workspace module graph and symbol analysis.

Parses the workspace's Rust sources with `syn` and emits reports about
dependencies: crate dependency graph, per-crate module graphs, symbol
tables, fan in/out stats, cycle detection, and heatmaps.

## Binary

```
notgraph [--output docs/graph] [--top 10] [--fail-on-cycles]
```

Writes `report.json`, a Markdown report, and an HTML report into the
output directory (default `docs/graph`); `--fail-on-cycles` makes the run
fail when module cycles are detected.

## Modules

- `crate_graph` — `CrateGraph` built from cargo metadata workspace members
  and their inter-crate deps (nodes = crate names, edges = (from, to)).
- `module_graph` — per-crate module graph from `syn::ItemMod` traversal of
  each `src/**/*.rs` file (edges connect parent module → child module).
- `symbols` — per-crate `SymbolTable` of `Symbol { mod_path, kind, name,
is_pub }` for structs, enums, traits, fns, types and consts.
- `analysis` — fan stats (`fan_in`/`fan_out`), cycle detection (Kahn's
  algorithm), hotspots/heatmap stats, `GraphStats` and `top` top-N entries.
- `emit` — `write_all` writes the JSON / Markdown / HTML reports.
- `types` — graph data types (`CrateGraph`, `ModuleGraph`, `Symbol`,
  `SymbolKind`, `SymbolTable`, `FanStats`, `Hotspot`, `HotspotKind`,
  `GraphStats`).

## Testing

`cargo test -p notgraph`
