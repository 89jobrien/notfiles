use crate::types::{GraphStats, HotspotKind, SymbolKind, SymbolTable};
use anyhow::Result;
use std::path::Path;

pub fn write_all(output_dir: &Path, stats: &GraphStats, symbol_tables: &[SymbolTable]) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    write_json(output_dir, stats)?;
    write_markdown(output_dir, stats, symbol_tables)?;
    write_html(output_dir, stats)?;
    Ok(())
}

fn write_json(dir: &Path, stats: &GraphStats) -> Result<()> {
    std::fs::write(dir.join("report.json"), serde_json::to_string_pretty(stats)?)?;
    Ok(())
}

fn write_markdown(dir: &Path, stats: &GraphStats, symbol_tables: &[SymbolTable]) -> Result<()> {
    let mut md = String::new();
    md.push_str("# notgraph Report\n\n");

    md.push_str("## Crate Dependency Graph\n\n");
    for fs in &stats.crate_graph {
        md.push_str(&format!("- **{}** (fan-in: {}, fan-out: {})\n", fs.name, fs.fan_in, fs.fan_out));
    }
    md.push('\n');

    md.push_str("## Module Graphs\n\n");
    for ms in &stats.module_graphs {
        let status = if ms.cycles.is_empty() { "OK" } else { "CYCLES DETECTED" };
        md.push_str(&format!(
            "### {} [{}]\n\n- Modules: {}\n- Cycles: {}\n\n",
            ms.krate, status, ms.nodes.len(), ms.cycles.len()
        ));
    }

    md.push_str("## Hotspots\n\n### Fan-in (most depended-upon)\n\n");
    md.push_str("| Name | Score |\n|------|-------|\n");
    for h in stats.hotspots.iter().filter(|h| h.kind == HotspotKind::FanIn) {
        md.push_str(&format!("| {} | {} |\n", h.name, h.score));
    }
    md.push('\n');

    md.push_str("### Fan-out (highest coupling)\n\n");
    md.push_str("| Name | Score |\n|------|-------|\n");
    for h in stats.hotspots.iter().filter(|h| h.kind == HotspotKind::FanOut) {
        md.push_str(&format!("| {} | {} |\n", h.name, h.score));
    }
    md.push('\n');

    md.push_str("## Public Symbol Inventory\n\n");
    for table in symbol_tables {
        md.push_str(&format!("### {}\n\n", table.krate));
        for kind in [SymbolKind::Struct, SymbolKind::Enum, SymbolKind::Trait, SymbolKind::Fn, SymbolKind::Type, SymbolKind::Const] {
            let syms: Vec<&str> = table.symbols.iter()
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

fn write_html(dir: &Path, stats: &GraphStats) -> Result<()> {
    let nodes_js: String = stats.crate_graph.iter().enumerate()
        .map(|(i, n)| format!("{{id:{},label:{:?},title:'fan-in:{} fan-out:{}'}}", i, n.name, n.fan_in, n.fan_out))
        .collect::<Vec<_>>()
        .join(",");

    let cycle_html = if stats.cycles.is_empty() {
        "<p>No cycles detected.</p>".to_string()
    } else {
        stats.cycles.iter()
            .map(|c| format!("<p>CYCLE: {}</p>", c.join(" -&gt; ")))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let hotspot_rows: String = stats.hotspots.iter()
        .map(|h| format!("<tr><td>{}</td><td>{}</td><td>{}</td></tr>", h.name, h.kind, h.score))
        .collect::<Vec<_>>()
        .join("\n");

    let html = format!(
        include_str!("../templates/report.html.template"),
        nodes_js = nodes_js,
        hotspot_rows = hotspot_rows,
        cycle_html = cycle_html,
    );

    std::fs::write(dir.join("report.html"), html)?;
    Ok(())
}
