# notgraph

Workspace module graph and symbol analysis for the `notfiles` Cargo workspace.

`notgraph` walks the workspace's `Cargo.toml` manifests and source trees, builds a crate
dependency graph and a per-crate module graph, computes fan-in/fan-out and cycle stats, and
emits JSON, Markdown, and HTML (Mermaid) reports.

- Library crate: `notgraph_lib` (`src/lib.rs`)
- Binary: `notgraph` (`src/main.rs`)

## Install / Build

```bash
cargo build -p notgraph
```

## Usage

```bash
cargo run -p notgraph -- [OPTIONS]
```

| Flag                | Default          | Description                                              |
| -------------------- | ---------------- | --------------------------------------------------------- |
| `--output <OUTPUT>`  | `target/notgraph`| Directory the reports are written to                       |
| `--fail-on-cycles`   | off              | Exit with a non-zero status and print detected cycles      |
| `--top <TOP>`        | `10`             | Number of top fan-in/fan-out hotspots to include per graph |

`notgraph` locates the workspace root by walking up from the current directory to find the
workspace `Cargo.toml`, so it can be run from anywhere inside the repo.

Example — regenerate reports and fail the run if a dependency cycle exists:

```bash
cargo run -p notgraph -- --fail-on-cycles
```

## Output

`write_all` (`src/emit.rs`) writes three files into the output directory:

| File           | Contents                                                                 |
| -------------- | ------------------------------------------------------------------------- |
| `report.json`  | `GraphStats` (crate fan stats, per-crate module stats, hotspots, cycles) serialized as pretty JSON |
| `report.md`    | Human-readable Markdown: crate dependency graph, per-crate module graph status, fan-in/fan-out hotspot tables, public symbol inventory, cycle report |
| `report.html`  | Self-contained HTML page with Mermaid diagrams: a crate dependency flowchart (grouped into `Core`/`Features`/`Tools` subgraphs, heatmap-colored by fan-in, cycle nodes highlighted in red) and one module-graph diagram per crate |

The HTML report is rendered from the template at `templates/report.html.template`.

## How it works

1. **`crate_graph::build`** — runs `cargo_metadata` against the workspace manifest (`--no-deps`)
   and builds a `CrateGraph` of workspace-member crate names and their intra-workspace
   dependency edges.
2. For every workspace member with a `src/` directory:
   - **`module_graph::build`** — walks the crate's source tree with `syn` to build a
     `ModuleGraph` of `mod`/`use` relationships between modules.
   - **`symbols::build`** — walks the same tree to build a `SymbolTable` of public `struct`,
     `enum`, `trait`, `fn`, `type`, and `const` items per module.
3. **`analysis::analyse`** — computes fan-in/fan-out (`fan_stats`), detects cycles via Kahn's
   algorithm (`detect_cycles`) on both the crate graph and each module graph, and picks the top
   `--top` fan-in/fan-out hotspots (`hotspots`).
4. **`emit::write_all`** — writes `report.json`, `report.md`, and `report.html` to the output
   directory.

If `--fail-on-cycles` is set and any cycle was detected (at the crate level or within any
crate's module graph), `notgraph` prints each cycle to stderr and exits with status `1`.

## Library modules

| Module          | Purpose                                                                 |
| ---------------- | ------------------------------------------------------------------------- |
| `types`          | Shared data types (`CrateGraph`, `ModuleGraph`, `SymbolTable`, `GraphStats`, `Hotspot`, ...) and `path_to_mod`, which converts a source file path into a `crate::foo::bar`-style module path |
| `crate_graph`    | Builds the workspace crate dependency graph via `cargo_metadata`           |
| `module_graph`   | Builds a single crate's module graph by walking its source tree with `syn` |
| `symbols`        | Builds a single crate's public symbol table by walking its source tree with `syn` |
| `analysis`       | Fan-in/fan-out stats, cycle detection (Kahn's algorithm), hotspot ranking  |
| `emit`           | Writes the JSON/Markdown/HTML reports                                      |

## Tests

```bash
cargo test -p notgraph
```

Unit tests live alongside their modules (`crate_graph.rs`, `analysis.rs`). Integration tests and
fixtures live in `tests/integration.rs` and `tests/fixtures/`.

## CI integration

The workspace CI (`.github/workflows/ci.yml`) runs a `notgraph` job on every push and pull
request:

```yaml
- name: Generate & check module graph
  run: cargo run -p notgraph -- --fail-on-cycles
- name: Upload graph reports
  uses: actions/upload-artifact@v4
  with:
    name: notgraph-reports
    path: target/notgraph
```

The job fails the build if a dependency cycle is detected, and uploads the generated reports as
a build artifact.
