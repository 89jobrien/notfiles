use anyhow::{Context, Result};
use clap::Parser;
use notgraph_lib::{analysis, crate_graph, emit, module_graph, symbols};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "notgraph",
    about = "Workspace module graph and symbol analysis"
)]
struct Cli {
    #[arg(long, default_value = "docs/graph")]
    output: PathBuf,

    #[arg(long)]
    fail_on_cycles: bool,

    #[arg(long, default_value_t = 10)]
    top: usize,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let manifest = find_workspace_manifest()?;
    let workspace_root = manifest.parent().unwrap().to_path_buf();

    let crate_g = crate_graph::build(&manifest).context("failed to build crate graph")?;

    let meta = cargo_metadata::MetadataCommand::new()
        .manifest_path(&manifest)
        .no_deps()
        .exec()?;

    let mut module_graphs = Vec::new();
    let mut symbol_tables = Vec::new();

    for pkg in &meta.packages {
        if !meta.workspace_members.contains(&pkg.id) {
            continue;
        }
        let src_dir = PathBuf::from(pkg.manifest_path.as_str())
            .parent()
            .unwrap()
            .join("src");
        if !src_dir.exists() {
            continue;
        }
        module_graphs.push(
            module_graph::build(pkg.name.clone(), &src_dir)
                .with_context(|| format!("module_graph failed for {}", pkg.name))?,
        );
        symbol_tables.push(
            symbols::build(pkg.name.clone(), &src_dir)
                .with_context(|| format!("symbols failed for {}", pkg.name))?,
        );
    }

    let stats = analysis::analyse(&crate_g, &module_graphs, cli.top);

    let output_dir = if cli.output.is_absolute() {
        cli.output.clone()
    } else {
        workspace_root.join(&cli.output)
    };

    emit::write_all(&output_dir, &crate_g, &stats, &symbol_tables)
        .context("failed to write reports")?;

    println!("notgraph: reports written to {}", output_dir.display());

    if cli.fail_on_cycles && !stats.cycles.is_empty() {
        eprintln!("notgraph: {} cycle(s) detected", stats.cycles.len());
        for cycle in &stats.cycles {
            eprintln!("  {}", cycle.join(" -> "));
        }
        std::process::exit(1);
    }

    Ok(())
}

fn find_workspace_manifest() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            let content = std::fs::read_to_string(&candidate)?;
            if content.contains("[workspace]") {
                return Ok(candidate);
            }
        }
        anyhow::ensure!(dir.pop(), "could not find workspace Cargo.toml");
    }
}
