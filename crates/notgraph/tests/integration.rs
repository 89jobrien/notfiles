use notgraph_lib::{analysis, module_graph, symbols};
use std::path::Path;

fn fixtures(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .join("src")
}

#[test]
fn clean_fixture_has_no_cycles() {
    let mg = module_graph::build("clean".to_string(), &fixtures("clean")).unwrap();
    assert!(
        analysis::detect_cycles(&mg.nodes, &mg.edges).is_empty(),
        "expected no cycles; nodes={:?} edges={:?}",
        mg.nodes, mg.edges
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

#[test]
fn symbols_fixture_has_six_public_symbols() {
    let table = symbols::build("symbols".to_string(), &fixtures("symbols")).unwrap();
    let pub_count = table.symbols.iter().filter(|s| s.is_pub).count();
    assert_eq!(
        pub_count, 6,
        "expected 6 public symbols (Foo, Bar, Baz, qux, Alias, VALUE), got: {:?}",
        table.symbols.iter().filter(|s| s.is_pub).map(|s| &s.name).collect::<Vec<_>>()
    );
}
