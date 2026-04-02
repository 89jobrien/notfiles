# HANDOFF.md

State of the `notfiles` repo as of 2026-04-01.

## What Was Done

### Previous session (2026-03-31)

Restructured from a flat `src/` layout into a Cargo workspace of 5 crates:
- `notcore` — shared library (types, config, paths, errors)
- `notfiles` — dotfiles linker, migrated from `src/`
- `notsecrets` — age key retrieval (Bitwarden/file/prompt) + SOPS decrypt
- `nothooks` — Nushell hook runner with phase/state tracking
- `notstrap` — new-machine bootstrap orchestrator

### This session (2026-04-01)

Began age-native redesign of `notsecrets` — replacing sops shell-out and Bitwarden CLI dependency with a self-contained age encryption/decryption library implemented in pure Rust (modeled after rage, no dependency on it).

**Spec and plan written:**
- `docs/superpowers/specs/2026-04-01-notsecrets-age-redesign.md`
- `docs/superpowers/plans/2026-04-01-notsecrets-age-redesign.md`

**Implementation progress (Tasks 1–7 of 12 complete):**

| Task | Status | Commit |
|------|--------|--------|
| 1: Scaffold (Cargo.toml, error.rs, domain types, legacy shim) | ✅ | `34d9e3d` |
| 2: format.rs (age wire format parser/serializer) | ✅ | `2476daa` |
| 3: X25519 identity + recipient | ✅ | `6202c5d` |
| 4: Scrypt identity + recipient | ✅ | `b899198` |
| 5: SSH Ed25519 identity + recipient | ✅ | `566501c` |
| 6: SSH RSA identity + recipient | ✅ | `a0fe6f7` |
| 7: EncryptedIdentity (stub, real impl needs Decryptor) | ✅ | `545a45b` |
| 8: Encryptor | ⏳ not started |  |
| 9: Decryptor + wire EncryptedIdentity | ⏳ not started |  |
| 10: sources/ migration (IdentitySource trait) | ⏳ not started |  |
| 11: resolve_identities() public API | ⏳ not started |  |
| 12: notstrap migration + cargo check --workspace | ⏳ not started |  |

**Current state:**
- `notsecrets` compiles and passes all existing tests
- Legacy API (`AgeKeySource`, `resolve_age_key`, `install_age_key`, `decrypt_sops`) preserved in `_legacy.rs` — `notstrap` still works
- `EncryptedIdentity::unwrap_file_key` is a stub returning an error until `Decryptor` is wired in Task 9

## Resuming

To continue the age-native redesign, pick up at Task 8 (Encryptor). The plan is at:
`docs/superpowers/plans/2026-04-01-notsecrets-age-redesign.md`

Tasks 8–12 need to be executed in order (each depends on the previous). Use the subagent-driven-development skill to dispatch implementer subagents per task.

Key context for resuming:
- `EncryptedIdentity` stub in `src/identities/encrypted.rs` must have its `unwrap_file_key` body replaced after `Decryptor` exists (end of Task 9)
- The `EnvInjector` type in `notstrap` changes from `Box<dyn Fn>` to `Box<dyn FnOnce>` in Task 12
- `_legacy.rs` is deleted in Task 10

## Pending Issues (carried from previous session)

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
