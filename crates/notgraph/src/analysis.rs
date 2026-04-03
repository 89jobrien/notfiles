use crate::types::{CrateGraph, FanStats, GraphStats, Hotspot, HotspotKind, ModStats, ModuleGraph};
use std::collections::{HashMap, VecDeque};

pub fn fan_stats(nodes: &[String], edges: &[(String, String)]) -> Vec<FanStats> {
    let mut fan_in: HashMap<&str, usize> = nodes.iter().map(|n| (n.as_str(), 0)).collect();
    let mut fan_out: HashMap<&str, usize> = nodes.iter().map(|n| (n.as_str(), 0)).collect();
    for (from, to) in edges {
        *fan_out.entry(from.as_str()).or_insert(0) += 1;
        *fan_in.entry(to.as_str()).or_insert(0) += 1;
    }
    nodes
        .iter()
        .map(|n| FanStats {
            name: n.clone(),
            fan_in: *fan_in.get(n.as_str()).unwrap_or(&0),
            fan_out: *fan_out.get(n.as_str()).unwrap_or(&0),
        })
        .collect()
}

/// Kahn's algorithm. Returns cycle participants if graph is not a DAG.
pub fn detect_cycles(nodes: &[String], edges: &[(String, String)]) -> Vec<Vec<String>> {
    let mut in_degree: HashMap<&str, usize> = nodes.iter().map(|n| (n.as_str(), 0)).collect();
    let mut adj: HashMap<&str, Vec<&str>> = nodes.iter().map(|n| (n.as_str(), vec![])).collect();

    let node_set: std::collections::HashSet<&str> = nodes.iter().map(|n| n.as_str()).collect();
    for (from, to) in edges {
        if node_set.contains(from.as_str()) && node_set.contains(to.as_str()) {
            *in_degree.entry(to.as_str()).or_insert(0) += 1;
            adj.entry(from.as_str()).or_default().push(to.as_str());
        }
    }

    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter(|&(_, &d)| d == 0)
        .map(|(n, _)| *n)
        .collect();

    let mut visited = 0usize;
    while let Some(node) = queue.pop_front() {
        visited += 1;
        for &neighbour in adj.get(node).unwrap_or(&vec![]) {
            let deg = in_degree.entry(neighbour).or_insert(0);
            *deg -= 1;
            if *deg == 0 {
                queue.push_back(neighbour);
            }
        }
    }

    if visited == nodes.len() {
        return vec![];
    }

    let cycle_nodes: Vec<String> = in_degree
        .iter()
        .filter(|&(_, &d)| d > 0)
        .map(|(n, _)| n.to_string())
        .collect();

    vec![cycle_nodes]
}

pub fn hotspots(stats: &[FanStats], top_n: usize) -> Vec<Hotspot> {
    let mut result = Vec::new();

    let mut by_fan_in = stats.to_vec();
    by_fan_in.sort_by(|a, b| b.fan_in.cmp(&a.fan_in));
    for s in by_fan_in.iter().take(top_n) {
        if s.fan_in > 0 {
            result.push(Hotspot {
                name: s.name.clone(),
                kind: HotspotKind::FanIn,
                score: s.fan_in,
            });
        }
    }

    let mut by_fan_out = stats.to_vec();
    by_fan_out.sort_by(|a, b| b.fan_out.cmp(&a.fan_out));
    for s in by_fan_out.iter().take(top_n) {
        if s.fan_out > 0 {
            result.push(Hotspot {
                name: s.name.clone(),
                kind: HotspotKind::FanOut,
                score: s.fan_out,
            });
        }
    }

    result
}

pub fn analyse(
    crate_graph: &CrateGraph,
    module_graphs: &[ModuleGraph],
    top_n: usize,
) -> GraphStats {
    let crate_stats = fan_stats(&crate_graph.nodes, &crate_graph.edges);
    let crate_hotspots = hotspots(&crate_stats, top_n);

    let mut all_cycles: Vec<Vec<String>> = Vec::new();
    let mut mod_stats_list: Vec<ModStats> = Vec::new();

    for mg in module_graphs {
        let node_stats = fan_stats(&mg.nodes, &mg.edges);
        let cycles = detect_cycles(&mg.nodes, &mg.edges);
        all_cycles.extend(cycles.clone());
        mod_stats_list.push(ModStats {
            krate: mg.krate.clone(),
            nodes: node_stats,
            cycles,
        });
    }

    let all_mod_fan: Vec<FanStats> = mod_stats_list
        .iter()
        .flat_map(|ms| ms.nodes.iter().cloned())
        .collect();
    let mod_hotspots = hotspots(&all_mod_fan, top_n);

    let mut all_hotspots = crate_hotspots;
    all_hotspots.extend(mod_hotspots);

    GraphStats {
        crate_graph: crate_stats,
        module_graphs: mod_stats_list,
        hotspots: all_hotspots,
        cycles: all_cycles,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_stats_counts_correctly() {
        let nodes = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let edges = vec![
            ("a".to_string(), "b".to_string()),
            ("a".to_string(), "c".to_string()),
            ("b".to_string(), "c".to_string()),
        ];
        let stats = fan_stats(&nodes, &edges);
        let a = stats.iter().find(|s| s.name == "a").unwrap();
        let c = stats.iter().find(|s| s.name == "c").unwrap();
        assert_eq!(a.fan_out, 2);
        assert_eq!(a.fan_in, 0);
        assert_eq!(c.fan_in, 2);
        assert_eq!(c.fan_out, 0);
    }

    #[test]
    fn no_cycle_in_acyclic_graph() {
        let nodes = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let edges = vec![
            ("a".to_string(), "b".to_string()),
            ("b".to_string(), "c".to_string()),
        ];
        assert!(detect_cycles(&nodes, &edges).is_empty());
    }

    #[test]
    fn detects_cycle_in_cyclic_graph() {
        let nodes = vec!["a".to_string(), "b".to_string()];
        let edges = vec![
            ("a".to_string(), "b".to_string()),
            ("b".to_string(), "a".to_string()),
        ];
        let cycles = detect_cycles(&nodes, &edges);
        assert!(!cycles.is_empty());
    }

    #[test]
    fn hotspots_picks_top_n_by_fan_in_and_fan_out() {
        let stats = vec![
            FanStats {
                name: "a".to_string(),
                fan_in: 5,
                fan_out: 1,
            },
            FanStats {
                name: "b".to_string(),
                fan_in: 3,
                fan_out: 2,
            },
            FanStats {
                name: "c".to_string(),
                fan_in: 1,
                fan_out: 4,
            },
        ];
        let spots = hotspots(&stats, 1);
        let fi = spots.iter().find(|h| h.kind == HotspotKind::FanIn).unwrap();
        let fo = spots
            .iter()
            .find(|h| h.kind == HotspotKind::FanOut)
            .unwrap();
        assert_eq!(fi.name, "a");
        assert_eq!(fo.name, "c");
    }
}
