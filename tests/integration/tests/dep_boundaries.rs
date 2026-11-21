//! Crate dependency boundaries for the workspace:
//!   - `notcore` stays a leaf (no workspace dependencies)
//!   - `notfiles`, `notsecrets` and `nothooks` may use `notcore` but not each other
//!   - `notstrap` is the orchestrator and is unrestricted
//!
//! Runs under `cargo test`, so CI enforces it with the rest of the suite.

use cargo_metadata::MetadataCommand;
use std::collections::{BTreeMap, BTreeSet};

const LEAF: &str = "notcore";
const FEATURE_CRATES: [&str; 3] = ["notfiles", "notsecrets", "nothooks"];

type Deps = BTreeMap<String, BTreeSet<String>>;

/// Human-readable violations of the boundary rules; empty when the graph is clean.
fn violations(deps: &Deps) -> Vec<String> {
    let mut found = Vec::new();

    if let Some(leaf_deps) = deps.get(LEAF).filter(|d| !d.is_empty()) {
        found.push(format!("{LEAF} depends on workspace crates: {leaf_deps:?}"));
    }

    for crate_name in FEATURE_CRATES {
        let siblings: BTreeSet<&String> = deps
            .get(crate_name)
            .into_iter()
            .flatten()
            .filter(|d| FEATURE_CRATES.contains(&d.as_str()))
            .collect();
        if !siblings.is_empty() {
            found.push(format!(
                "{crate_name} depends on sibling feature crates: {siblings:?}"
            ));
        }
    }

    found
}

fn graph(edges: &[(&str, &[&str])]) -> Deps {
    edges
        .iter()
        .map(|(name, deps)| {
            (
                name.to_string(),
                deps.iter().map(|d| d.to_string()).collect(),
            )
        })
        .collect()
}

#[test]
fn clean_graph_has_no_violations() {
    let deps = graph(&[
        ("notcore", &[]),
        ("notfiles", &["notcore"]),
        ("notsecrets", &["notcore"]),
        ("nothooks", &["notcore"]),
        (
            "notstrap",
            &["notcore", "notfiles", "nothooks", "notsecrets"],
        ),
    ]);
    assert!(violations(&deps).is_empty());
}

#[test]
fn a_leaf_with_workspace_deps_is_reported() {
    let deps = graph(&[("notcore", &["notfiles"]), ("notfiles", &[])]);
    let found = violations(&deps);
    assert_eq!(found.len(), 1);
    assert!(found[0].starts_with("notcore depends on workspace crates"));
}

#[test]
fn feature_crates_depending_on_each_other_are_reported() {
    let deps = graph(&[
        ("notcore", &[]),
        ("notfiles", &["notcore", "nothooks"]),
        ("nothooks", &["notcore"]),
    ]);
    let found = violations(&deps);
    assert_eq!(found.len(), 1);
    assert!(found[0].contains("notfiles depends on sibling feature crates"));
}

#[test]
fn the_orchestrator_may_depend_on_anything() {
    let deps = graph(&[(
        "notstrap",
        &["notfiles", "nothooks", "notsecrets", "notcore"],
    )]);
    assert!(violations(&deps).is_empty());
}

#[test]
fn the_real_workspace_respects_the_boundaries() {
    let metadata = MetadataCommand::new()
        .no_deps()
        .exec()
        .expect("cargo metadata should run");
    let members: BTreeSet<String> = metadata.packages.iter().map(|p| p.name.clone()).collect();

    let deps: Deps = metadata
        .packages
        .iter()
        .map(|pkg| {
            let internal = pkg
                .dependencies
                .iter()
                .filter(|d| members.contains(&d.name))
                .map(|d| d.name.clone())
                .collect();
            (pkg.name.clone(), internal)
        })
        .collect();

    let found = violations(&deps);
    assert!(
        found.is_empty(),
        "boundary violations:\n  {}",
        found.join("\n  ")
    );
}
