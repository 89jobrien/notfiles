use crate::types::{CrateGraph, GraphStats, HotspotKind, ModuleGraph, SymbolKind, SymbolTable};
use anyhow::Result;
use std::path::Path;

pub fn write_all(
    output_dir: &Path,
    crate_graph: &CrateGraph,
    module_graphs: &[ModuleGraph],
    stats: &GraphStats,
    symbol_tables: &[SymbolTable],
) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    write_json(output_dir, stats)?;
    write_markdown(output_dir, stats, symbol_tables)?;
    write_html(output_dir, crate_graph, module_graphs, stats, symbol_tables)?;
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

fn write_html(
    dir: &Path,
    crate_graph: &CrateGraph,
    module_graphs: &[ModuleGraph],
    stats: &GraphStats,
    symbol_tables: &[SymbolTable],
) -> Result<()> {
    let fan_map: std::collections::HashMap<&str, (usize, usize)> = stats
        .crate_graph
        .iter()
        .map(|fs| (fs.name.as_str(), (fs.fan_in, fs.fan_out)))
        .collect();

    // Symbol counts per crate
    let sym_count: std::collections::HashMap<&str, usize> = symbol_tables
        .iter()
        .map(|t| {
            let count = t.symbols.iter().filter(|s| s.is_pub).count();
            (t.krate.as_str(), count)
        })
        .collect();

    // Nodes involved in any cycle (for red highlighting)
    let cycle_nodes: std::collections::HashSet<&str> = stats
        .cycles
        .iter()
        .flat_map(|c| c.iter().map(|n| n.as_str()))
        .collect();

    // Max fan-in for heatmap normalisation
    let max_fan_in = stats
        .crate_graph
        .iter()
        .map(|fs| fs.fan_in)
        .max()
        .unwrap_or(1)
        .max(1);

    // Heatmap: interpolate between dim blue and bright blue based on fan-in
    let heatmap_color = |fan_in: usize| -> String {
        let t = fan_in as f32 / max_fan_in as f32;
        let r = (30.0 + t * 20.0) as u8;
        let g = (58.0 + t * 80.0) as u8;
        let b = (95.0 + t * 160.0) as u8;
        format!("#{:02x}{:02x}{:02x}", r, g, b)
    };

    // Classify nodes into subgraphs by topology
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
        let syms = sym_count.get(n).copied().unwrap_or(0);
        format!(
            "{}[\"{}<br/>in:{} out:{} | {} pub\"]",
            n, n, fi, fo, syms
        )
    };

    // ── Crate dependency graph ──────────────────────────────────────────────
    let mut mermaid = String::from("flowchart LR\n");

    if !leaves.is_empty() {
        mermaid.push_str("  subgraph Core\n    direction TB\n");
        for n in &leaves { mermaid.push_str(&format!("    {}\n", node_label(n))); }
        mermaid.push_str("  end\n");
    }
    if !features.is_empty() {
        mermaid.push_str("  subgraph Features\n    direction TB\n");
        for n in &features { mermaid.push_str(&format!("    {}\n", node_label(n))); }
        mermaid.push_str("  end\n");
    }
    if !roots.is_empty() {
        mermaid.push_str("  subgraph Tools\n    direction TB\n");
        for n in &roots { mermaid.push_str(&format!("    {}\n", node_label(n))); }
        mermaid.push_str("  end\n");
    }

    for (from, to) in &crate_graph.edges {
        mermaid.push_str(&format!("  {} --> {}\n", to, from));
    }

    // Heatmap styles
    for fs in &stats.crate_graph {
        let color = heatmap_color(fs.fan_in);
        mermaid.push_str(&format!(
            "  style {} fill:{},stroke:#4a9eff,color:#e0e0e0\n",
            fs.name, color
        ));
    }

    // Cycle highlights (override heatmap)
    for n in &cycle_nodes {
        mermaid.push_str(&format!(
            "  style {} fill:#7a1a1a,stroke:#f44336,color:#ffffff\n",
            n
        ));
    }

    let graph_diagram = mermaid;

    // Build cycle lookup from stats (ModuleGraph itself has no cycle data)
    let mod_cycles: std::collections::HashMap<&str, &Vec<Vec<String>>> = stats
        .module_graphs
        .iter()
        .map(|ms| (ms.krate.as_str(), &ms.cycles))
        .collect();

    // ── Per-crate module graphs ─────────────────────────────────────────────
    let mut module_diagrams = String::new();
    for mg in module_graphs {
        // Collect cycle nodes for this crate
        let empty: Vec<Vec<String>> = Vec::new();
        let cycles = mod_cycles.get(mg.krate.as_str()).copied().unwrap_or(&empty);
        let crate_cycle_nodes: std::collections::HashSet<&str> = cycles
            .iter()
            .flat_map(|c| c.iter().map(|n| n.as_str()))
            .collect();

        let mut d = format!("flowchart TD\n");

        // Nodes: strip crate prefix for readable labels, skip ::tests modules
        for node in &mg.nodes {
            let short = node
                .strip_prefix(&format!("{}::", mg.krate))
                .unwrap_or(node.as_str());
            // Sanitize id for Mermaid (replace :: with __)
            let id = node.replace("::", "__");
            d.push_str(&format!("  {}[\"{}\"]\n", id, short));
        }

        // Edges
        for (from, to) in &mg.edges {
            let from_id = from.replace("::", "__");
            let to_id = to.replace("::", "__");
            d.push_str(&format!("  {} --> {}\n", from_id, to_id));
        }

        // Cycle highlights
        for n in &crate_cycle_nodes {
            let id = n.replace("::", "__");
            d.push_str(&format!(
                "  style {} fill:#7a1a1a,stroke:#f44336,color:#ffffff\n",
                id
            ));
        }

        module_diagrams.push_str(&format!(
            "<h3>{}</h3>\n<div class=\"graph-wrap\"><pre class=\"mermaid\">\n%%{{init: {{\"theme\":\"dark\"}}}}\n{}</pre></div>\n",
            mg.krate, d
        ));
    }

    let cycle_html = if stats.cycles.is_empty() {
        "<p>No cycles detected.</p>".to_string()
    } else {
        stats
            .cycles
            .iter()
            .map(|c| format!("<p class=\"cycle-entry\">CYCLE: {}</p>", c.join(" &rarr; ")))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let hotspot_rows: String = stats
        .hotspots
        .iter()
        .map(|h| format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            h.name, h.kind, h.score
        ))
        .collect::<Vec<_>>()
        .join("\n");

    let html = format!(
        include_str!("../templates/report.html.template"),
        graph_diagram = graph_diagram,
        module_diagrams = module_diagrams,
        hotspot_rows = hotspot_rows,
        cycle_html = cycle_html,
    );

    std::fs::write(dir.join("report.html"), html)?;
    Ok(())
}
