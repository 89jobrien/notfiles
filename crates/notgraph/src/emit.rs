use crate::types::{CrateGraph, GraphStats, HotspotKind, SymbolKind, SymbolTable};
use anyhow::Result;
use std::path::Path;

pub fn write_all(
    output_dir: &Path,
    crate_graph: &CrateGraph,
    stats: &GraphStats,
    symbol_tables: &[SymbolTable],
) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    write_json(output_dir, stats)?;
    write_markdown(output_dir, stats, symbol_tables)?;
    write_html(output_dir, crate_graph, stats)?;
    Ok(())
}

fn write_json(dir: &Path, stats: &GraphStats) -> Result<()> {
    std::fs::write(
        dir.join("report.json"),
        serde_json::to_string_pretty(stats)?,
    )?;
    Ok(())
}

fn write_markdown(dir: &Path, stats: &GraphStats, symbol_tables: &[SymbolTable]) -> Result<()> {
    let mut md = String::new();
    md.push_str("# notgraph Report\n\n");

    md.push_str("## Crate Dependency Graph\n\n");
    for fs in &stats.crate_graph {
        md.push_str(&format!(
            "- **{}** (fan-in: {}, fan-out: {})\n",
            fs.name, fs.fan_in, fs.fan_out
        ));
    }
    md.push('\n');

    md.push_str("## Module Graphs\n\n");
    for ms in &stats.module_graphs {
        let status = if ms.cycles.is_empty() {
            "OK"
        } else {
            "CYCLES DETECTED"
        };
        md.push_str(&format!(
            "### {} [{}]\n\n- Modules: {}\n- Cycles: {}\n\n",
            ms.krate,
            status,
            ms.nodes.len(),
            ms.cycles.len()
        ));
    }

    md.push_str("## Hotspots\n\n### Fan-in (most depended-upon)\n\n");
    md.push_str("| Name | Score |\n|------|-------|\n");
    for h in stats
        .hotspots
        .iter()
        .filter(|h| h.kind == HotspotKind::FanIn)
    {
        md.push_str(&format!("| {} | {} |\n", h.name, h.score));
    }
    md.push('\n');

    md.push_str("### Fan-out (highest coupling)\n\n");
    md.push_str("| Name | Score |\n|------|-------|\n");
    for h in stats
        .hotspots
        .iter()
        .filter(|h| h.kind == HotspotKind::FanOut)
    {
        md.push_str(&format!("| {} | {} |\n", h.name, h.score));
    }
    md.push('\n');

    md.push_str("## Public Symbol Inventory\n\n");
    for table in symbol_tables {
        md.push_str(&format!("### {}\n\n", table.krate));
        for kind in [
            SymbolKind::Struct,
            SymbolKind::Enum,
            SymbolKind::Trait,
            SymbolKind::Fn,
            SymbolKind::Type,
            SymbolKind::Const,
        ] {
            let syms: Vec<&str> = table
                .symbols
                .iter()
                .filter(|s| s.kind == kind && s.is_pub)
                .map(|s| s.name.as_str())
                .collect();
            if !syms.is_empty() {
                md.push_str(&format!("**{}**: {}\n\n", kind, syms.join(", ")));
            }
        }
    }

    md.push_str("## Cycle Report\n\n");
    if stats.cycles.is_empty() {
        md.push_str("No cycles detected.\n");
    } else {
        for cycle in &stats.cycles {
            md.push_str(&format!("- CYCLE: {}\n", cycle.join(" -> ")));
        }
    }

    std::fs::write(dir.join("report.md"), md)?;
    Ok(())
}

fn write_html(dir: &Path, crate_graph: &CrateGraph, stats: &GraphStats) -> Result<()> {
    let fan_map: std::collections::HashMap<&str, (usize, usize)> = stats
        .crate_graph
        .iter()
        .map(|fs| (fs.name.as_str(), (fs.fan_in, fs.fan_out)))
        .collect();

    // Classify nodes into subgraphs by topology:
    //   leaf    = no outgoing edges to workspace crates (fan-out == 0)
    //   root    = no incoming edges from workspace crates (fan-in == 0)
    //   feature = everything else
    let leaf_nodes: std::collections::HashSet<&str> = stats
        .crate_graph
        .iter()
        .filter(|fs| fs.fan_out == 0)
        .map(|fs| fs.name.as_str())
        .collect();
    let root_nodes: std::collections::HashSet<&str> = stats
        .crate_graph
        .iter()
        .filter(|fs| fs.fan_in == 0)
        .map(|fs| fs.name.as_str())
        .collect();

    let mut leaves: Vec<&str> = crate_graph.nodes.iter()
        .map(|n| n.as_str())
        .filter(|n| leaf_nodes.contains(n) && !root_nodes.contains(n))
        .collect();
    let mut roots: Vec<&str> = crate_graph.nodes.iter()
        .map(|n| n.as_str())
        .filter(|n| root_nodes.contains(n) && !leaf_nodes.contains(n))
        .collect();
    let mut features: Vec<&str> = crate_graph.nodes.iter()
        .map(|n| n.as_str())
        .filter(|n| !leaf_nodes.contains(n) && !root_nodes.contains(n))
        .collect();
    // Nodes that are both leaf and root (isolated, e.g. notgraph) go into roots
    let mut isolated: Vec<&str> = crate_graph.nodes.iter()
        .map(|n| n.as_str())
        .filter(|n| leaf_nodes.contains(n) && root_nodes.contains(n))
        .collect();
    leaves.sort();
    roots.sort();
    features.sort();
    isolated.sort();
    roots.extend(isolated);

    let node_label = |n: &str| -> String {
        let (fi, fo) = fan_map.get(n).copied().unwrap_or((0, 0));
        format!("{}[\"{}<br/>in:{} out:{}\"]", n, n, fi, fo)
    };

    // Mermaid flowchart: dependency -> dependent (LR)
    let mut mermaid = String::from("flowchart LR\n");

    if !leaves.is_empty() {
        mermaid.push_str("  subgraph Core\n    direction TB\n");
        for n in &leaves {
            mermaid.push_str(&format!("    {}\n", node_label(n)));
        }
        mermaid.push_str("  end\n");
    }
    if !features.is_empty() {
        mermaid.push_str("  subgraph Features\n    direction TB\n");
        for n in &features {
            mermaid.push_str(&format!("    {}\n", node_label(n)));
        }
        mermaid.push_str("  end\n");
    }
    if !roots.is_empty() {
        mermaid.push_str("  subgraph Tools\n    direction TB\n");
        for n in &roots {
            mermaid.push_str(&format!("    {}\n", node_label(n)));
        }
        mermaid.push_str("  end\n");
    }

    for (from, to) in &crate_graph.edges {
        // dependency -> dependent
        mermaid.push_str(&format!("  {} --> {}\n", to, from));
    }
    let graph_diagram = mermaid;

    let cycle_html = if stats.cycles.is_empty() {
        "<p>No cycles detected.</p>".to_string()
    } else {
        stats
            .cycles
            .iter()
            .map(|c| format!("<p>CYCLE: {}</p>", c.join(" -&gt; ")))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let hotspot_rows: String = stats
        .hotspots
        .iter()
        .map(|h| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                h.name, h.kind, h.score
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let html = format!(
        include_str!("../templates/report.html.template"),
        graph_diagram = graph_diagram,
        hotspot_rows = hotspot_rows,
        cycle_html = cycle_html,
    );

    std::fs::write(dir.join("report.html"), html)?;
    Ok(())
}
