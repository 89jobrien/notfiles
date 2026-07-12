#!/usr/bin/env python3
"""
Enforce crate dependency boundaries for the notfiles workspace:
  - notcore must remain a leaf crate (no deps on other notfiles crates)
  - notfiles, notsecrets, nothooks may depend on notcore but NOT on each other
  - notstrap may depend on notcore + feature crates (it's the orchestrator)

Exits 0 if boundaries are clean, 1 if violations found.
"""
import json
import subprocess
import sys

LEAF = "notcore"
FEATURE_CRATES = {"notfiles", "notsecrets", "nothooks"}
ORCHESTRATOR = "notstrap"
WORKSPACE_CRATES = FEATURE_CRATES | {LEAF, ORCHESTRATOR}

result = subprocess.run(
    ["cargo", "metadata", "--no-deps", "--format-version", "1"],
    capture_output=True, text=True, check=True,
)
meta = json.loads(result.stdout)

# Build name -> deps map for workspace members only
crate_deps: dict[str, set[str]] = {}
for pkg in meta["packages"]:
    name = pkg["name"]
    if name not in WORKSPACE_CRATES:
        continue
    deps = {d["name"] for d in pkg["dependencies"] if d["name"] in WORKSPACE_CRATES}
    crate_deps[name] = deps

violations = []

# notcore must have no workspace deps
if crate_deps.get(LEAF):
    violations.append(f"{LEAF} depends on workspace crates: {crate_deps[LEAF]}")

# feature crates must not depend on each other
for crate in FEATURE_CRATES:
    forbidden = crate_deps.get(crate, set()) & FEATURE_CRATES
    if forbidden:
        violations.append(f"{crate} depends on sibling feature crates: {forbidden}")

if violations:
    print("FAIL: crate dependency boundary violations detected:")
    for v in violations:
        print(f"  - {v}")
    sys.exit(1)

print("OK: crate dependency boundaries are clean")
print(f"  {LEAF}: no workspace deps (leaf)")
for c in sorted(FEATURE_CRATES):
    print(f"  {c}: depends only on {LEAF}")
print(f"  {ORCHESTRATOR}: orchestrator (unrestricted)")
