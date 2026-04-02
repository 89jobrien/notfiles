use crate::types::{CrateName, ModPath, ModuleGraph};
use anyhow::Result;
use std::path::Path;
use syn::visit::Visit;
use walkdir::WalkDir;

struct ModCollector {
    current_mod: ModPath,
    edges: Vec<(ModPath, ModPath)>,
    nodes: Vec<ModPath>,
}

impl ModCollector {
    fn new(root_mod: ModPath) -> Self {
        Self {
            current_mod: root_mod.clone(),
            edges: Vec::new(),
            nodes: vec![root_mod],
        }
    }
}

impl<'ast> Visit<'ast> for ModCollector {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let child = format!("{}::{}", self.current_mod, node.ident);
        if !self.nodes.contains(&child) {
            self.nodes.push(child.clone());
        }
        self.edges.push((self.current_mod.clone(), child.clone()));
        let parent = std::mem::replace(&mut self.current_mod, child);
        syn::visit::visit_item_mod(self, node);
        self.current_mod = parent;
    }
}

fn path_to_mod(krate: &str, rel: &Path) -> ModPath {
    let mut parts: Vec<String> = vec![krate.to_string()];
    for component in rel.components() {
        let s = component.as_os_str().to_string_lossy();
        if s == "lib.rs" || s == "main.rs" {
            break;
        }
        let s = s.trim_end_matches(".rs").to_string();
        if s != "mod" {
            parts.push(s);
        }
    }
    parts.join("::")
}

pub fn build(krate: CrateName, src_dir: &Path) -> Result<ModuleGraph> {
    let mut all_nodes: Vec<ModPath> = Vec::new();
    let mut all_edges: Vec<(ModPath, ModPath)> = Vec::new();

    for entry in WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |x| x == "rs"))
    {
        let path = entry.path();
        let content = std::fs::read_to_string(path)?;
        let file = syn::parse_file(&content)?;
        let rel = path.strip_prefix(src_dir)?;
        let mod_path = path_to_mod(&krate, rel);

        let mut collector = ModCollector::new(mod_path);
        collector.visit_file(&file);

        for node in collector.nodes {
            if !all_nodes.contains(&node) {
                all_nodes.push(node);
            }
        }
        all_edges.extend(collector.edges);
    }

    all_edges.sort();
    all_edges.dedup();

    Ok(ModuleGraph { krate, nodes: all_nodes, edges: all_edges })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_fixture_has_expected_nodes() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/clean/src");
        let graph = build("clean".to_string(), &fixture).unwrap();
        assert!(graph.nodes.contains(&"clean".to_string()));
        assert!(graph.nodes.contains(&"clean::alpha".to_string()));
        assert!(graph.nodes.contains(&"clean::beta".to_string()));
    }

    #[test]
    fn clean_fixture_has_edges_from_lib() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/clean/src");
        let graph = build("clean".to_string(), &fixture).unwrap();
        assert!(graph.edges.contains(&("clean".to_string(), "clean::alpha".to_string())));
        assert!(graph.edges.contains(&("clean".to_string(), "clean::beta".to_string())));
    }
}
