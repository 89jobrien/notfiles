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
fn cyclic_fixture_detects_module_references() {
    // The "cyclic" fixture demonstrates a case where foo and bar mutually
    // reference each other's names through mod declarations. While Rust's
    // module system itself is acyclic (hierarchical), this tests that
    // module_graph properly captures all module declarations.
    let mg = module_graph::build("cyclic".to_string(), &fixtures("cyclic")).unwrap();

    // Should find nodes for both foo and bar at top level
    assert!(mg.nodes.contains(&"cyclic::foo".to_string()));
    assert!(mg.nodes.contains(&"cyclic::bar".to_string()));

    // Should find the nested modules they declare
    assert!(mg.nodes.contains(&"cyclic::foo::bar".to_string()));
    assert!(mg.nodes.contains(&"cyclic::bar::foo".to_string()));

    // Should have edges for all declarations
    assert!(mg.edges.contains(&("cyclic".to_string(), "cyclic::foo".to_string())));
    assert!(mg.edges.contains(&("cyclic".to_string(), "cyclic::bar".to_string())));
    assert!(mg.edges.contains(&("cyclic::foo".to_string(), "cyclic::foo::bar".to_string())));
    assert!(mg.edges.contains(&("cyclic::bar".to_string(), "cyclic::bar::foo".to_string())));
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
