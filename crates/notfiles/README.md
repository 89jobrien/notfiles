# notfiles

A modern dotfiles manager — pure Rust alternative to GNU Stow.

Symlinks (or copies) files from organized "package" directories into a
target location (typically `~`). Supports include/exclude filtering,
platform gates, and multiple output formats.

## CLI

```
notfiles [--dir <dir>] [--dry-run] [--verbose|-v] [--json] <command>
```

Subcommands: `init`, `link`, `unlink`, `status`, `check`, `diff`, `adopt`,
`completions`.

## Core flow (`link`)

```
main → config.validate()
     → resolve_packages_filtered (include/exclude + platform filter)
     → collect_files (recursive walk with ignore filtering)
     → linker::link_package (symlink/copy via FileStore port; records in
       .notfiles-state.toml; returns LinkResult counters)
```

## Modules

- `adapters` — `FileStoreImpl` (real fs), `InMemoryFileStore` (testing),
  `TerminalReporter`, `JsonReporter` (NDJSON).
- `cli` — clap parsing and dispatch.
- `detect` — auto-detects stow/chezmoi/notfiles repos
  (`DetectedManager`, `ManagerKind`, `DetectedPackage`).
- `ignore` — `IgnoreMatcher`: globset-based ignore matching, also applied
  per path component (for directory-level ignores).
- `linker` — `State` (`.notfiles-state.toml`), `StateEntry`, `LinkOptions`,
  `LinkResult`, `adopt_files`.
- `package` — package discovery/resolution
  (`discover_packages_filtered`, `resolve_packages_filtered_with_store`)
  and file collection.
- `ports` — `FileStore` trait (fs abstraction).
- `status` — `package_status` / `diff_package` reporting
  (`FileStatus`: linked/copied/missing/conflict/orphan).

## Usage (library)

```rust
use notfiles::{link, LinkOptions};
use std::path::Path;

let opts = LinkOptions {
    force: false,
    no_backup: false,
    dry_run: false,
    verbose: true,
};
let state = link(Path::new("/home/user/dotfiles"), &[], &opts)?;
```

`link_with_store` / `unlink_with_store` take injectable `FileStore` and
`Reporter` for testing.

## Testing

`cargo test -p notfiles`
