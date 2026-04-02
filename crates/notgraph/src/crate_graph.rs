use crate::types::{CrateGraph, CrateName};
use anyhow::Result;

pub fn build(manifest_path: &std::path::Path) -> Result<CrateGraph> {
    let meta = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_path)
        .no_deps()
        .exec()?;

    let workspace_members: std::collections::HashSet<String> = meta
        .workspace_members
        .iter()
        .filter_map(|id| meta.packages.iter().find(|p| &p.id == id))
        .map(|p| p.name.clone())
        .collect();

    let nodes: Vec<CrateName> = workspace_members.iter().cloned().collect();

    let mut edges: Vec<(CrateName, CrateName)> = Vec::new();
    for pkg in meta.packages.iter().filter(|p| workspace_members.contains(&p.name)) {
        for dep in &pkg.dependencies {
            if workspace_members.contains(&dep.name) {
                edges.push((pkg.name.clone(), dep.name.clone()));
            }
        }
    }

    Ok(CrateGraph { nodes, edges })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_crate_graph_for_workspace() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .parent().unwrap()
            .join("Cargo.toml");

        let graph = build(&manifest).unwrap();

        assert!(graph.nodes.contains(&"notgraph".to_string()));
        assert!(graph.nodes.contains(&"notcore".to_string()));
        assert!(graph.edges.contains(&("notstrap".to_string(), "notfiles".to_string())));
        assert!(!graph.edges.iter().any(|(from, _)| from == "notcore"));
    }
}
