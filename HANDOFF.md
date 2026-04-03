# HANDOFF.md

State of the `notfiles` repo as of 2026-04-03.

## What Was Done

### Session 1 (2026-03-31)

Restructured from a flat `src/` layout into a Cargo workspace of 5 crates:
- `notcore` — shared library (types, config, paths, errors)
- `notfiles` — dotfiles linker, migrated from `src/`
- `notsecrets` — age key retrieval (Bitwarden/file/prompt) + SOPS decrypt
- `nothooks` — Nushell hook runner with phase/state tracking
- `notstrap` — new-machine bootstrap orchestrator

### Session 2 (2026-04-01)

Completed the age-native redesign of `notsecrets` — all 12 tasks done.

**All tasks complete:**

| Task | Status | Commit |
|------|--------|--------|
| 1: Scaffold (Cargo.toml, error.rs, domain types, legacy shim) | ✅ | `34d9e3d` |
| 2: format.rs (age wire format parser/serializer) | ✅ | `2476daa` |
| 3: X25519 identity + recipient | ✅ | `6202c5d` |
| 4: Scrypt identity + recipient | ✅ | `b899198` |
| 5: SSH Ed25519 identity + recipient | ✅ | `566501c` |
| 6: SSH RSA identity + recipient | ✅ | `a0fe6f7` |
| 7: EncryptedIdentity (passphrase-protected identity file) | ✅ | `545a45b` |
| 8: Encryptor — multi-recipient age encryption | ✅ | `9863af6` |
| 9: Decryptor + wire EncryptedIdentity | ✅ | `e388d75` |
| 10+11: sources/ migration + resolve_identities() | ✅ | `f80ea02` |
| 12: notstrap migration, remove sops shell-out | ✅ | `58de007` |

**Current state:**
- 80 tests pass, 0 clippy warnings, `cargo check --workspace` clean
- `_legacy.rs` deleted; all deprecated shims removed
- `notsecrets` is now a self-contained age encryption/decryption library
- `notstrap` uses `resolve_identities` + `Decryptor` directly; no sops shell-out
- `EnvInjector` changed to `FnOnce`; `main.rs` sets `env_injector: None`
- Example at `crates/notsecrets/examples/age_smoke.rs` demonstrates round-trip encrypt/decrypt

Also done this session (outside notfiles):
- Fixed `~/.config/sops/age/keys.txt` — was `AGE-SECRET-KEY-1TESTKEY` placeholder, now real key from 1Password item `age-key-dotfiles` (UUID `6meypnchchq3tsb32mdnzxtlia`)
- `env.nu` now self-heals `keys.txt` from 1Password on fresh login if missing/placeholder
- Expanded `~/.secrets` to cover 18 keys (AWS, Docker, GitHub, Slack, ElevenLabs, Groq, Mistral, LangChain/Smith, SerpAPI, Twilio, Tavily, Context7) — all via `op://` UUID refs, resolved by `op inject` at shell startup
- Removed duplicate `op read` calls for OPENAI/ANTHROPIC from `env.nu`
- Cleaned up `~/.config/dev-bootstrap/secrets.env` — removed TAVILY/CONTEXT7 (now in `~/.secrets`), kept non-secret MCP config vars

## Pending Issues

### 1. Push to remote
Several commits ahead of `gitea/main`. Push when ready.

### 2. notstrap has an empty lib.rs
`crates/notstrap/src/lib.rs` is empty — created as a stub. Either delete `[lib]` from `notstrap/Cargo.toml` or remove the file.

### 3. notstrap config not documented
`notstrap.toml` format is defined inline via serde structs but never written down. A sample config would help.

### 4. Integration test coverage for nothooks assumes nu in PATH
`nothooks` integration tests call `nu` directly. CI needs `nu` installed or tests need `#[ignore]` + feature flag.

### 5. notstrap sops_file path footgun
If `sops_file` in `notstrap.toml` is an absolute path, the `join` will silently override the base. Low priority.

### 6. HuggingFace token not in ~/.secrets
No API token found in 1Password — only login credentials. Add token manually if needed.

### 7. notsecrets is not wire-compatible with standard age
The Encryptor/Decryptor use single-block ChaCha20-Poly1305 rather than the STREAM construction. Files encrypted by `notsecrets` cannot be decrypted by `rage`/`age` CLI and vice versa. This is intentional per the spec but worth documenting.

### Session 3 (2026-04-03)

**notgraph** — crate dependency + module graph report landed and polished:
- Heatmap coloring on crate nodes (fan-in intensity → blue gradient)
- Per-crate module graphs (Mermaid `flowchart TD`) with `__`-separated node IDs
- Cycle callouts with `cycle-entry` CSS class
- Fixed: hyphenated crate names (`my-crate`) were emitted as raw Mermaid IDs (invalid); added `mermaid_id` sanitizer (`-` → `_`) applied to all node IDs, edges, `style` directives, and cycle highlights
- 6 new integration tests covering: cyclic fixture, output file presence, hyphen sanitization, single-crate heatmap edge case, per-crate module graph rendering, cycle callout presence
- `notsecrets` refactor confirmed complete — no TODOs, no pending cleanup

**notgraph planned extensions** (not yet implemented — prioritized list):

| Priority | Section | Data source | Visualization |
|----------|---------|-------------|---------------|
| 1 | Code coverage heatmap | `cargo llvm-cov --json` | Treemap or per-crate bar |
| 2 | Cyclomatic complexity | `rust-code-analysis-cli --metrics` | Histogram + top-offenders table |
| 3 | Lines of code breakdown | `tokei --output json` | Stacked bar (code/comments/blanks) |
| 4 | Unsafe code audit | `cargo geiger --output-format Json` | Table + traffic-light badges |
| 5 | Dep freshness + vulns | `cargo outdated --format json` + `cargo audit --json` | Semver-distance dots + advisory cards |
| 6 | Doc coverage | `cargo doc -Z unstable-options --show-coverage` (nightly) | Horizontal bars per crate |
| 7 | Commit churn heatmap | `git log --name-only` | Scatter: churn × complexity × LOC × coverage |
| 8 | API surface area | `cargo rustdoc --output-format json` | Bar by item kind per crate |
| 9 | Test-to-code ratio | `tokei` + `#[test]` count | Table + stacked bar |
| 10 | Dep weight/bloat | `cargo metadata` | Stacked bar (direct+transitive) + duplicates |

Easiest to add with no extra tooling: LOC breakdown (tokei), test-to-code ratio, dep weight (cargo metadata always available).
Coverage + complexity require `cargo-llvm-cov` and `rust-code-analysis-cli` installed.
