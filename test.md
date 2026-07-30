## Unreleased

### <!-- 0 --> Features

- Add shared types, config, paths, error crate
- Migrate notfiles into crates/ workspace layout
- Add age key retrieval with bw/file/prompt sources
- Add phase-aware hook runner with setup-hook state tracking
- Add new-machine bootstrap orchestrator
- Scaffold age-native domain types and trait ports
- Age wire format parser/serializer with unit tests
- X25519 identity and recipient with wrap/unwrap
- Scrypt identity and recipient (passphrase-based)
- SSH Ed25519 identity and recipient
- SSH RSA identity and recipient (OAEP-SHA256)
- EncryptedIdentity stub — passphrase-protected identity file
- Encryptor — multi-recipient age encryption
- Decryptor — identity-driven age decryption with MAC verification
- Migrate sources to IdentitySource trait, add resolve_identities, remove legacy API
- Migrate to notsecrets age-native API, remove sops shell-out
- Scaffold crate with lib+bin targets
- Define all shared types
- Implement crate_graph collector
- Implement module_graph collector
- Implement symbols extractor
- Implement analysis (fan stats, Kahn cycles, hotspots)
- Implement emit (md, json, html)
- Wire full CLI pipeline
- Wire all:graph and all:graph-check into mise CI
- Group crates into Core/Features/Tools subgraphs
- Heatmap coloring, symbol counts, per-crate module graphs, cycle callouts
- Add notnet crate and YubikeySource, thread Tailscale into notstrap
- Add multi-provider secret resolution system
- Add nu_libs autoload package
- Adopt full nushell config from dotfiles
- Add detect subcommand for stow/chezmoi/notfiles auto-detection
- Add x alias for cargo xtask
- Hexagonal architecture refactor with adapters, integration tests, and notsecrets cleanup
- Add Gitea API foundation

### <!-- 1 --> Bug Fixes

- Replace truncated Apache license with full text
- Replace Apache license with canonical text from apache.org
- Use full name Joseph O'Brien in license files
- Add .nothooks-state.toml to default_ignore and starter_toml
- Fix clippy warning and unwrap panics in lib extraction
- Address blocking sentinel findings (unsafe SAFETY comment, unwrap panics)
- Address security findings from devkit-review (#1–#7)
- Phantom node cycles, inline mod symbols, dedup path_to_mod
- Render dep graph edges, add dark mode
- Reverse edge direction to dependency -> dependent
- Replace vis.js with Mermaid for clean static flowchart
- Add closing %% to module graph mermaid init directives
- Replace :: with / in module graph node labels to fix Mermaid parse error
- Sanitize hyphenated crate IDs in Mermaid output; add graph feature integration tests
- Save state on partial link failure; skip re-copy of unchanged files
- Wire age identity to decryption; fix parse_env_line quoting
- Validate package on unlink; batch state I/O in HookRunner
- Atomic state file writes via temp-file + rename
- Make preflight.nu script executable (#2)
- Install nushell for integration tests

### <!-- 10 --> Other

- Fix Bitwarden password exposure and ChaCha nonce hardening
- Fix unlink validation and HookRunner state batching (closes #10, #13)
- Audit Command call sites; confirm no argument injection
- Formalize ports and update related files

### <!-- 2 --> Refactor

- Extract run() into lib with BootstrapOptions
- Make hook runner language-agnostic
- Formalize FileStore and IdentitySource ports in notfiles
- Replace EnvInjector with SecretResolver

### <!-- 3 --> Documentation

- Add workspace architecture design and README
- Add workspace implementation plan
- Update plan and spec — replace sh with nu for hook scripts
- Update CLAUDE.md for workspace layout, add HANDOFF.md
- Add CI section to CLAUDE.md, gitignore .claude.local.md
- Add GitHub publishing and identity sections to CLAUDE.md
- Move GitHub/identity sections to global CLAUDE.md
- Add integration tests design spec
- Add integration tests implementation plan
- Note absorption of pj into notfiles workspace
- Add notsecrets age-native redesign spec
- Add notsecrets age-native implementation plan
- Update HANDOFF.md — age-native redesign progress (tasks 1–7 complete)
- Update HANDOFF.md — all 12 tasks complete, secrets pipeline notes
- Update HANDOFF for session 3, add notgraph extension roadmap, ignore librust_out.rlib
- Add notgraph implementation plan
- Create HANDOFF.yaml — initial session handoff with 3 open items
- Mark notfile-1 and notfile-2 done in handoff
- Mark all notgraph plan tasks complete; close notfile-3
- Fix garbled <important> block, add notgraph crate to workspace table
- Update handoff
- Add Tailscale integration design spec
- Update handoff
- Log 2026-07-08 — hexagonal architecture refactor, adapters, integration tests, notsecrets cleanup

### <!-- 5 --> Styling

- Replace emoji with [OK] in workflow summary; cargo fmt integration tests

### <!-- 6 --> Testing

- Add bootstrap flow tests via notstrap::run()
- Improve test clarity with expect() and constants
- Add cross-crate boundary tests
- Improve cross_crate test clarity
- Add integration tests for clean/cyclic/symbols fixtures
- Fix cyclic integration test to use synthetic graph

### <!-- 7 --> Miscellaneous Tasks

- Ignore .worktrees directory
- Add release profile, clean up old files
- Add public repo readiness workflow
- Add dual MIT/Apache-2.0 license, update Cargo.toml and README
- Add integration test crate to workspace
- Add .remember/ to gitignore
- Standardize CI workflows and git hooks
- Add just workspace recipe
- Add affected-crate release workflow
- Add TODO roadmap, nf alias, and install recipe
- Move config payload out of tool repo
- Sort Cargo.toml dependencies
- Stop tracking macOS artifacts
- Remove nested agent worktree marker

### Commit Statistics

- 108 commit(s) contributed to the release.
- 103 day(s) passed between the first and last commit.
- 108 commit(s) parsed as conventional.
- 0 linked issue(s) detected in commits.

<!-- generated by git-cliff -->
