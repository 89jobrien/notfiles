use notgraph_lib::{analysis, emit, module_graph, symbols, types};
use std::path::Path;
use tempfile::TempDir;

fn fixture_src(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .join("src")
}

#[test]
fn clean_fixture_has_no_cycles() {
    let mg = module_graph::build("clean".to_string(), &fixture_src("clean")).unwrap();
    assert!(
        analysis::detect_cycles(&mg.nodes, &mg.edges).is_empty(),
        "expected no cycles; nodes={:?} edges={:?}",
        mg.nodes,
        mg.edges
    );
}

#[test]
fn detect_cycles_finds_cycles_in_synthetic_graph() {
    let nodes = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let edges = vec![
        ("a".to_string(), "b".to_string()),
        ("b".to_string(), "c".to_string()),
        ("c".to_string(), "a".to_string()), // creates a->b->c->a cycle
    ];
    let cycles = analysis::detect_cycles(&nodes, &edges);
    assert!(
        !cycles.is_empty(),
        "expected detect_cycles to find the a->b->c->a cycle, got empty"
    );
}

/// The cyclic fixture's module graph builds without error and has the expected
/// cross-module containment structure. The cycle detection test using synthetic
/// data covers the algorithm; this test confirms the builder handles mutual
/// `pub mod` declarations without panicking.
#[test]
fn cyclic_fixture_module_graph_builds() {
    let mg = module_graph::build("cyclic".to_string(), &fixture_src("cyclic")).unwrap();
    assert!(
        mg.nodes.contains(&"cyclic".to_string()),
        "expected root module node 'cyclic'"
    );
    // foo and bar are declared at the top level in lib.rs
    assert!(
        mg.nodes.contains(&"cyclic::foo".to_string()),
        "expected cyclic::foo node"
    );
    assert!(
        mg.nodes.contains(&"cyclic::bar".to_string()),
        "expected cyclic::bar node"
    );
}

/// write_all produces report.html, report.md, and report.json in the output dir.
#[test]
fn write_all_creates_expected_files() {
    use types::{CrateGraph, GraphStats, ModuleGraph};

    let out = TempDir::new().unwrap();
    let crate_graph = CrateGraph {
        nodes: vec!["alpha".to_string(), "beta".to_string()],
        edges: vec![("beta".to_string(), "alpha".to_string())],
    };
    let module_graphs: Vec<ModuleGraph> = vec![];
    let stats = analysis::analyse(&crate_graph, &module_graphs, 3);
    let symbol_tables: Vec<types::SymbolTable> = vec![];

    emit::write_all(
        out.path(),
        &crate_graph,
        &module_graphs,
        &stats,
        &symbol_tables,
    )
    .unwrap();

    assert!(
        out.path().join("report.html").exists(),
        "report.html missing"
    );
    assert!(out.path().join("report.md").exists(), "report.md missing");
    assert!(
        out.path().join("report.json").exists(),
        "report.json missing"
    );
}

/// Crate names with hyphens must not produce malformed Mermaid output — the
/// generated HTML must contain a sanitized node ID (hyphen replaced with _).
#[test]
fn write_all_sanitizes_hyphenated_crate_names() {
    use types::{CrateGraph, ModuleGraph};

    let out = TempDir::new().unwrap();
    let crate_graph = CrateGraph {
        nodes: vec!["my-core".to_string(), "my-app".to_string()],
        edges: vec![("my-app".to_string(), "my-core".to_string())],
    };
    let module_graphs: Vec<ModuleGraph> = vec![];
    let stats = analysis::analyse(&crate_graph, &module_graphs, 3);
    let symbol_tables: Vec<types::SymbolTable> = vec![];

    emit::write_all(
        out.path(),
        &crate_graph,
        &module_graphs,
        &stats,
        &symbol_tables,
    )
    .unwrap();

    let html = std::fs::read_to_string(out.path().join("report.html")).unwrap();
    // Raw hyphenated names must not appear as unquoted node IDs
    assert!(
        !html.contains("my-core["),
        "hyphenated ID 'my-core[' must not appear in Mermaid — use my_core"
    );
    assert!(
        html.contains("my_core["),
        "sanitized ID 'my_core[' must appear in Mermaid output"
    );
}

/// Heatmap: a single-crate graph (max_fan_in = 0 or 1) must not panic and must
/// produce a style directive in the HTML output.
#[test]
fn write_all_heatmap_does_not_panic_for_single_crate() {
    use types::{CrateGraph, ModuleGraph};

    let out = TempDir::new().unwrap();
    let crate_graph = CrateGraph {
        nodes: vec!["solo".to_string()],
        edges: vec![],
    };
    let module_graphs: Vec<ModuleGraph> = vec![];
    let stats = analysis::analyse(&crate_graph, &module_graphs, 3);
    let symbol_tables: Vec<types::SymbolTable> = vec![];

    emit::write_all(
        out.path(),
        &crate_graph,
        &module_graphs,
        &stats,
        &symbol_tables,
    )
    .unwrap();

    let html = std::fs::read_to_string(out.path().join("report.html")).unwrap();
    assert!(
        html.contains("style solo fill:"),
        "heatmap style directive missing for solo crate"
    );
}

/// Per-crate module graphs: write_all renders a <h3> section for each
/// ModuleGraph entry in the HTML output.
#[test]
fn write_all_renders_per_crate_module_graphs() {
    use types::{CrateGraph, ModuleGraph};

    let out = TempDir::new().unwrap();
    let mg = module_graph::build("clean".to_string(), &fixture_src("clean")).unwrap();
    let crate_graph = CrateGraph {
        nodes: vec!["clean".to_string()],
        edges: vec![],
    };
    let stats = analysis::analyse(&crate_graph, &[mg.clone()], 3);
    let symbol_tables: Vec<types::SymbolTable> = vec![];

    emit::write_all(out.path(), &crate_graph, &[mg], &stats, &symbol_tables).unwrap();

    let html = std::fs::read_to_string(out.path().join("report.html")).unwrap();
    assert!(
        html.contains("<h3>clean</h3>"),
        "expected per-crate <h3>clean</h3> section in HTML"
    );
    assert!(
        html.contains("flowchart TD"),
        "expected flowchart TD directive for module graph"
    );
}

/// Cycle callouts: when the crate graph contains cycles, the HTML must include
/// the cycle-entry element with the cycle nodes.
#[test]
fn write_all_cycle_callout_appears_in_html() {
    use types::{CrateGraph, ModuleGraph};

    let out = TempDir::new().unwrap();
    // Build a two-node crate graph with a synthetic cycle
    let crate_graph = CrateGraph {
        nodes: vec!["alpha".to_string(), "beta".to_string()],
        edges: vec![
            ("alpha".to_string(), "beta".to_string()),
            ("beta".to_string(), "alpha".to_string()),
        ],
    };
    let module_graphs: Vec<ModuleGraph> = vec![];
    let stats = analysis::analyse(&crate_graph, &module_graphs, 3);
    let symbol_tables: Vec<types::SymbolTable> = vec![];

    emit::write_all(
        out.path(),
        &crate_graph,
        &module_graphs,
        &stats,
        &symbol_tables,
    )
    .unwrap();

    let html = std::fs::read_to_string(out.path().join("report.html")).unwrap();
    assert!(
        html.contains("cycle-entry"),
        "expected cycle-entry element in HTML when cycles are present"
    );
}

#[test]
fn symbols_fixture_has_six_public_symbols() {
    let table = symbols::build("symbols".to_string(), &fixture_src("symbols")).unwrap();
    let pub_count = table.symbols.iter().filter(|s| s.is_pub).count();
    assert_eq!(
        pub_count,
        6,
        "expected 6 public symbols (Foo, Bar, Baz, qux, Alias, VALUE), got: {:?}",
        table
            .symbols
            .iter()
            .filter(|s| s.is_pub)
            .map(|s| &s.name)
            .collect::<Vec<_>>()
    );
}
