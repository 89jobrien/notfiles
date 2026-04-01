# notgraph — Module Graph & Symbol Extraction Tooling

**Date:** 2026-04-01
**Status:** Approved

## Overview

A new `notgraph` binary-only workspace crate that analyses the notfiles workspace and produces three output artefacts: a Markdown summary, a JSON report, and a self-contained interactive HTML file. It runs in CI to enforce zero intra-crate module cycles, and locally for codebase exploration.

## Architecture

Three pipeline stages:

1. **Collect** — `cargo metadata` → crate dep graph; `walkdir` + `syn` → per-crate module trees and symbol tables
2. **Analyze** — fan-in/fan-out counts, intra-crate cycle detection (Kahn's algorithm), hotspot ranking
3. **Emit** — write `docs/graph/report.md`, `docs/graph/report.json`, `docs/graph/report.html`

`notgraph` is a binary-only crate. No other workspace crate depends on it.

## Components

Five modules in `crates/notgraph/src/`:

| Module | Responsibility |
|--------|---------------|
| `crate_graph` | Calls `cargo_metadata`, builds crate→crate dep map for workspace members only |
| `module_graph` | Walks `src/` per crate with `walkdir`, parses `mod` declarations via `syn` to build intra-crate module trees |
| `symbols` | Parses each `.rs` file with `syn`, extracts public structs/enums/traits/fns/types/consts by module |
| `analysis` | Fan-in/fan-out counts, cycle detection (Kahn's algorithm), hotspot ranking (top N by fan-in and fan-out separately) |
| `emit` | Writes `report.md`, `report.json`, `report.html` (self-contained inline CSS+JS via vis.js) |

`main.rs` wires collect → analyze → emit, then exits non-zero if cycles detected.

## Data Flow & Key Types

```
cargo metadata
    └─► CrateGraph { nodes: Vec<CrateName>, edges: Vec<(CrateName, CrateName)> }

walkdir + syn (per crate)
    └─► ModuleGraph { crate: CrateName, nodes: Vec<ModPath>, edges: Vec<(ModPath, ModPath)> }
    └─► SymbolTable { crate: CrateName, symbols: Vec<Symbol> }

Symbol { mod_path: ModPath, kind: SymbolKind, name: String, is_pub: bool }
SymbolKind: Struct | Enum | Trait | Fn | Type | Const

analysis
    └─► GraphStats {
          crate_graph:   FanStats,
          module_graphs: Vec<ModStats>,
          hotspots:      Vec<Hotspot>,   // top N by fan-in and fan-out separately
          cycles:        Vec<Vec<ModPath>>, // empty = clean
        }

FanStats  { name: String, fan_in: usize, fan_out: usize }
ModStats  { crate: CrateName, nodes: Vec<FanStats>, cycles: Vec<Vec<ModPath>> }
Hotspot   { name: String, kind: HotspotKind, score: usize }
HotspotKind: FanIn | FanOut
```

## CLI

```
notgraph [OPTIONS]

Options:
  --output <DIR>      Directory to write report.{md,json,html} [default: docs/graph]
  --fail-on-cycles    Exit 1 if any intra-crate module cycles are detected
  --top <N>           Number of hotspots to surface per category [default: 10]
```

## Output Files

**`report.json`** — full `GraphStats` serialised with `serde_json`. Machine-readable, suitable for diffing in CI.

**`report.md`** — sections:
- Crate dependency graph (adjacency list)
- Per-crate module graph summary (node count, edge count, cycle count)
- Hotspot tables: top N fan-in and top N fan-out for both crates and modules
- Public symbol inventory per crate (grouped by `SymbolKind`)
- Cycle report (empty section = ✅ clean)

**`report.html`** — self-contained single file:
- vis.js network diagram of the crate graph (interactive, filterable)
- Collapsible per-crate module graph diagrams
- Sortable hotspot tables
- No external network requests (vis.js inlined)

## Dependencies

```toml
[dependencies]
cargo_metadata = "0.18"
syn            = { version = "2", features = ["full", "visit"] }
walkdir        = "2"
serde          = { version = "1", features = ["derive"] }
serde_json     = "1"
anyhow         = "1"
```

## Testing

**Unit tests** (in each module): Kahn's algorithm with known graphs, fan-count arithmetic, hotspot ranking.

**Integration tests** (`crates/notgraph/tests/integration.rs`) with three fixture sets in `tests/fixtures/`:

| Fixture | Purpose |
|---------|---------|
| `clean/` | No cycles, known fan-in/out — assert output matches expected values |
| `cyclic/` | Two modules that mutually declare each other — assert cycle detected, exit code 1 with `--fail-on-cycles` |
| `symbols/` | Known public symbols — assert symbol table matches expected inventory |

## CI Wiring (`mise.toml`)

```toml
[tasks."all:graph"]
description = "Generate module graph reports"
run = "cargo run -p notgraph --release -- --output docs/graph"

[tasks."all:graph-check"]
description = "CI: fail if module cycles detected"
run = "cargo run -p notgraph --release -- --output docs/graph --fail-on-cycles"
```

`all:ci` gains `all:graph-check` after the existing `all:dep-boundaries` step.

## CI Behaviour

- Exit 0: no cycles (hotspots are informational only)
- Exit 1: one or more intra-crate module cycles detected (only with `--fail-on-cycles`)
- `docs/graph/` is `.gitignore`d — reports are generated, not committed

## Non-Goals

- Cross-crate module graph (crate boundaries are already enforced by `check-dep-boundaries.py`)
- Lifetime or type-level analysis
- Incremental/watch mode
