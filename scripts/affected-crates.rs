#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! cargo_metadata = "0.18"
//! ```
//!
//! Lists the workspace crates affected by a set of changed files: every crate that owns a
//! changed file, plus everything that depends on one transitively. A change outside every
//! crate (CI config, lockfile, ...) affects all crates. The `integration` test crate is
//! never reported.
//!
//! Usage: rust-script scripts/affected-crates.rs <changed-files.txt> <affected-crates.txt>

use anyhow::{Context, Result, bail};
use cargo_metadata::MetadataCommand;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;

const IGNORED: [&str; 1] = ["integration"];

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let (Some(changed_path), Some(out_path)) = (args.next(), args.next()) else {
        bail!("usage: affected-crates.rs <changed-files.txt> <affected-crates.txt>");
    };

    let changed: Vec<String> = fs::read_to_string(&changed_path)
        .with_context(|| format!("reading {changed_path}"))?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();

    let metadata = MetadataCommand::new().no_deps().exec()?;
    let root = metadata.workspace_root.as_str().to_string();

    // crate name -> directory relative to the workspace root
    let mut dirs: BTreeMap<String, String> = BTreeMap::new();
    for pkg in &metadata.packages {
        let dir = pkg
            .manifest_path
            .parent()
            .context("manifest path has no parent")?
            .as_str()
            .trim_start_matches(&root)
            .trim_start_matches('/')
            .to_string();
        dirs.insert(pkg.name.clone(), dir);
    }

    let mut reverse_deps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for pkg in &metadata.packages {
        for dep in &pkg.dependencies {
            if dirs.contains_key(&dep.name) {
                reverse_deps
                    .entry(dep.name.clone())
                    .or_default()
                    .insert(pkg.name.clone());
            }
        }
    }

    let mut direct: BTreeSet<String> = BTreeSet::new();
    let mut shared_change = false;
    for file in &changed {
        let owners: Vec<&String> = dirs
            .iter()
            .filter(|(_, dir)| !dir.is_empty() && (file == *dir || file.starts_with(&format!("{dir}/"))))
            .map(|(name, _)| name)
            .collect();
        if owners.is_empty() {
            // The release workflow file itself does not count as a shared change.
            if file != ".github/workflows/release.yml" {
                shared_change = true;
            }
        } else {
            direct.extend(owners.into_iter().cloned());
        }
    }
    if shared_change {
        direct.extend(dirs.keys().cloned());
    }

    let mut affected = direct.clone();
    let mut queue: VecDeque<String> = direct.into_iter().collect();
    while let Some(current) = queue.pop_front() {
        for dependent in reverse_deps.get(&current).into_iter().flatten() {
            if affected.insert(dependent.clone()) {
                queue.push_back(dependent.clone());
            }
        }
    }

    let affected: Vec<String> = affected
        .into_iter()
        .filter(|name| !IGNORED.contains(&name.as_str()))
        .collect();
    if affected.is_empty() {
        bail!("No affected crates detected since the last release tag.");
    }

    fs::write(&out_path, affected.join("\n") + "\n").with_context(|| format!("writing {out_path}"))?;
    Ok(())
}
