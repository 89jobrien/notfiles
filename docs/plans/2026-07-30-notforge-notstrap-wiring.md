# Plan: Wire notforge into notstrap

## Goal

Take `notforge` from "Gitea API adapter with unused ports" to "notstrap
can optionally ensure a Gitea repo exists and is wired as a git remote
during bootstrap" — the deferred goal named in the original forge
design doc ("notstrap integration deferred until the notforge API is
stable"). Existing bootstrap flows without a `[forge]` section must be
completely unaffected.

## Current State (verified against source, not the original design doc)

- `notforge::ports` already defines four traits: `ForgeApi` (implemented
  by `GiteaHttpApi<T: GiteaTransport>` in `gitea.rs`, backed by `ureq`),
  `ForgeLifecycle`, `GitRemoteManager`, `SecretResolverPort` — **the
  latter three have zero implementations anywhere in the crate.**
- `notforge::config` has `ForgeConfig { gitea: GiteaConfig, repositories:
  Vec<RepoSpec> }`, with `GiteaConfig.mode: GiteaMode` supporting
  `LocalProcess`, `LocalContainer`, `VmSystemd`, `Existing`.
- `notforge/src/main.rs` is a 3-line stub (`println!(VERSION)`); the
  `[[bin]]` target exists in `Cargo.toml` but does nothing yet.
- `notforge` depends on `notcore` only — no dependency on `notsecrets`
  or `notstrap`, and nothing depends on `notforge`.
- `notstrap::run(opts: BootstrapOptions) -> Result<Report>` loads
  `NotstrapConfig` (`bootstrap`, `tailscale: Option<...>`, `hooks: Vec<...>`)
  inline from TOML — no `[forge]` section exists today. The `tailscale`
  field is the precedent for "optional section, `None` = fully skipped."
- Secret resolution already happens at step 5b via
  `notsecrets::SecretResolver::from_config(..).resolve(key: &str) ->
  Result<Option<String>, SecretsError>` — a single-key resolve method
  exists (not just `resolve_all`), which is the natural shape for
  `SecretResolverPort::resolve`.
- `crates/notstrap/src/repo.rs` already shells out to `git` via
  `Command::new("git").arg(...).status()` for `clone_if_missing` — this
  is the established, audited precedent (see the 2026-04-11 argument-
  injection audit) for how `notforge::git`'s `CommandGitRemoteManager`
  should be implemented (discrete `.arg()` calls, no shell).

## Architecture

### Crates affected

| Crate | Changes |
|---|---|
| `notforge` | New `lifecycle.rs` (`ForgeLifecycle` impl for `Existing` mode only), new `git.rs` (`GitRemoteManager` via `git` CLI), orchestration functions in `lib.rs` |
| `notstrap` | New `SecretResolverPort` adapter, new optional `[forge]` config section, new optional bootstrap step, new dependency on `notforge` |

### New types and functions

| Item | Crate | Location |
|---|---|---|
| `ExistingGiteaLifecycle` | `notforge` | `src/lifecycle.rs` (new) |
| `CommandGitRemoteManager` | `notforge` | `src/git.rs` (new) |
| `resolve_auth`, `ensure_forge`, `ensure_repository`, `ssh_remote_url`, `ensure_git_remote` | `notforge` | `src/lib.rs` |
| `ForgeSection` (or reuse `notforge::ForgeConfig` shape — decided in Task 3) | `notstrap` | `src/lib.rs` |
| `NotsecretsResolverAdapter` (impl `notforge::ports::SecretResolverPort`) | `notstrap` | `src/forge.rs` (new) |

### Data flow (bootstrap, `[forge]` present and `enabled = true`)

```
run(opts)
  → load NotstrapConfig (now includes Option<ForgeSection>)
  → [existing] prereqs → tailscale
  → NEW: if forge.enabled:
        NotsecretsResolverAdapter::resolve(secret_ref)
          → notforge::resolve_auth(..)      (ForgeAuth)
          → notforge::ensure_forge(..)      (ForgeLifecycle::ensure_available)
          → notforge::ensure_repository(..) (ForgeApi::repo / create_repo)
        report.add("forge", Ok | Failed | Skipped-per-on_error)
  → [existing] clone dotfiles → age key → secrets → link → hooks → report
```

Forge wiring happens **before** clone, consistent with the "ensure the
repo exists, then clone it" use case — but only when explicitly enabled,
so it never inserts network/auth risk into the default path.

## Tech Stack

- No new external dependency beyond what's already in the workspace
  (`ureq`, already a `notforge` dependency, handles the Gitea HTTP
  calls; git remote/push operations shell out to the `git` binary,
  same as `notstrap::repo::clone_if_missing`).
- `notstrap/Cargo.toml` gains one new workspace-path dependency:
  `notforge = { workspace = true }`.

## Out of Scope

- `LocalProcess` / `LocalContainer` / `VmSystemd` `GiteaMode` variants —
  only `Existing` (a Gitea instance already reachable at `base_url`) is
  wired in this plan. The other three remain defined in config but
  return `NotforgeError::Command("unsupported gitea mode")` from
  `ForgeLifecycle` for now.
- `notforge`'s CLI (`main.rs`) — the binary target exists but stays a
  stub. notstrap only calls the `notforge` library.
- Token refresh/rotation — `ForgeAuth` is resolved once per bootstrap
  run, eagerly, not lazily.
- Composing `notnet`/Tailscale readiness with VM-mode forge — moot
  since VM mode is out of scope here.

---

## Tier 1 — notforge orchestration layer

### Task 1: `ForgeLifecycle` for `Existing` mode

**Crate**: `notforge`
**File(s)**: `crates/notforge/src/lifecycle.rs` (new), `src/lib.rs`
**Run**: `cargo nextest run -p notforge`

1. Write failing test in `lifecycle.rs`:
   ```rust
   #[test]
   fn existing_mode_verified_when_api_reachable() {
       let api = FakeForgeApi::reachable();
       let lifecycle = ExistingGiteaLifecycle::new(&api);
       let cfg = existing_mode_config();
       assert_eq!(lifecycle.ensure_available(&cfg).unwrap(), LifecycleStatus::Verified);
   }

   #[test]
   fn non_existing_mode_returns_unsupported() {
       let lifecycle = ExistingGiteaLifecycle::new(&FakeForgeApi::reachable());
       let cfg = local_process_mode_config();
       assert!(lifecycle.ensure_available(&cfg).is_err());
   }
   ```
   A `FakeForgeApi` (implementing `ForgeApi` with canned responses) needs
   to exist as a test helper — add it in `lifecycle.rs`'s `#[cfg(test)]`
   module, not a new pub type.

2. Implement `ExistingGiteaLifecycle<'a, A: ForgeApi>` wrapping a
   `&'a A`, implementing `ForgeLifecycle::ensure_available`:
   - `GiteaMode::Existing` → call `api.version()`; `Ok` → `Verified`,
     `Err` → propagate.
   - Any other `GiteaMode` variant → `Err(NotforgeError::Command(
     "gitea mode not yet supported by notstrap wiring".into()))`.

3. Verify: `cargo nextest run -p notforge` all green;
   `cargo clippy -p notforge -- -D warnings` zero.

4. Commit: `feat(notforge): implement ForgeLifecycle for Existing gitea mode`

### Task 2: `GitRemoteManager` via `git` CLI

**Crate**: `notforge`
**File(s)**: `crates/notforge/src/git.rs` (new), `src/lib.rs`
**Run**: `cargo nextest run -p notforge`

1. Write failing tests (use a temp git repo via `tempfile` +
   `git init`, same pattern the workspace already uses in
   `tests/integration/tests/bootstrap.rs`):
   ```rust
   #[test]
   fn set_remote_adds_when_missing() { /* git init tmp repo, set_remote, assert `git remote -v` shows it */ }

   #[test]
   fn set_remote_is_idempotent_when_url_matches() { /* set_remote twice, assert RemoteStatus::Unchanged on 2nd call */ }

   #[test]
   fn remote_url_returns_none_when_absent() { /* no remote configured, expect Ok(None) */ }
   ```

2. Implement `CommandGitRemoteManager` implementing `GitRemoteManager`:
   - `remote_url`: `git -C <repo> remote get-url <name>` — `Ok(None)`
     on nonzero exit (remote not found), `Ok(Some(url))` on success.
   - `set_remote`: check `remote_url` first; `git remote add` if absent,
     `git remote set-url` if present-and-different, no-op (`Unchanged`)
     if present-and-same.
   - `push`: `git -C <repo> push [--set-upstream] <name> <branch>`.
   - All commands built via discrete `.arg()` calls on
     `std::process::Command`, no shell — mirror the safety comment
     already present in `notstrap::repo::clone_if_missing`.

3. Verify and commit: `feat(notforge): implement GitRemoteManager via git CLI`

### Task 3: Orchestration functions in `notforge::lib`

**Crate**: `notforge`
**File(s)**: `crates/notforge/src/lib.rs`
**Run**: `cargo nextest run -p notforge`

1. Write failing integration-style tests in `lib.rs` (or
   `tests/orchestration.rs`) using fakes for all four ports:
   ```rust
   #[test]
   fn ensure_repository_creates_when_absent() { /* FakeForgeApi.repo() -> None, then create_repo() -> Some */ }

   #[test]
   fn ensure_repository_is_idempotent_when_present() { /* FakeForgeApi.repo() -> Some, create_repo never called */ }

   #[test]
   fn resolve_auth_prefers_token_over_basic() { /* ForgeAuthConfig with both set -> Token variant wins */ }
   ```

2. Implement, following the signatures already specified in
   `docs/designs/2026-07-08-on-demand-gitea-forge-design.md` (these are
   still accurate — only the lifecycle/git *adapters* were missing, not
   these function signatures):
   ```rust
   pub fn resolve_auth(config: &ForgeAuthConfig, secrets: &dyn SecretResolverPort) -> Result<ForgeAuth, NotforgeError>;
   pub fn ensure_forge(config: &ForgeConfig, lifecycle: &dyn ForgeLifecycle) -> Result<LifecycleStatus, NotforgeError>;
   pub fn ensure_repository(api: &dyn ForgeApi, auth: &ForgeAuth, spec: &RepoSpec) -> Result<RemoteRepository, NotforgeError>;
   pub fn ssh_remote_url(config: &GiteaConfig, repo: &RepoSpec) -> String;
   pub fn ensure_git_remote(manager: &dyn GitRemoteManager, local: &LocalRepository, repo: &RepoSpec, remote_url: &str) -> Result<RemoteStatus, NotforgeError>;
   ```
   `ensure_repository` checks `api.repo(owner, name)` first; only calls
   `create_repo` if `None`.

3. Verify and commit: `feat(notforge): add resolve_auth/ensure_forge/ensure_repository/ensure_git_remote orchestration`

---

## Tier 2 — notstrap wiring

### Task 4: `SecretResolverPort` adapter in notstrap

**Crate**: `notstrap`
**File(s)**: `crates/notstrap/src/forge.rs` (new), `Cargo.toml` (add `notforge` dependency)
**Run**: `cargo nextest run -p notstrap`

1. Add `notforge = { workspace = true }` to `crates/notstrap/Cargo.toml`.

2. Write failing test:
   ```rust
   #[test]
   fn adapter_resolves_via_notsecrets_env_source() {
       // build a SecretsConfig with an Env-provider binding
       // wrap in NotsecretsResolverAdapter, call .resolve(&ForgeSecretRef::Env{key})
       // assert it returns the value from std::env
   }
   ```

3. Implement:
   ```rust
   pub struct NotsecretsResolverAdapter<'a> {
       resolver: &'a notsecrets::SecretResolver,
   }

   impl<'a> notforge::ports::SecretResolverPort for NotsecretsResolverAdapter<'a> {
       fn resolve(&self, secret: &notforge::ForgeSecretRef) -> Result<String, notforge::NotforgeError> {
           let key = match secret {
               notforge::ForgeSecretRef::Env { key } => key,
               notforge::ForgeSecretRef::Op { uri } => uri, // routed through notsecrets' own op:// handling
           };
           self.resolver
               .resolve(key)
               .map_err(|e| notforge::NotforgeError::Secret(e.to_string()))?
               .ok_or_else(|| notforge::NotforgeError::Secret(format!("secret '{key}' not found")))
       }
   }
   ```
   Note: `ForgeSecretRef::Op { uri }` vs. `notsecrets`'s own provider
   model needs a small reconciliation — confirm during implementation
   whether `notsecrets::SecretResolver::resolve` takes a bound secret
   *name* (looked up in `SecretsConfig.secrets`) or a raw provider URI.
   If it's name-based, `ForgeSecretRef` needs to carry a name that maps
   to an existing `notsecrets.toml` binding, not a raw `op://` URI —
   resolve this ambiguity against `notsecrets/src/config.rs::SecretRef`
   before writing the adapter body.

4. Verify and commit: `feat(notstrap): add NotsecretsResolverAdapter for notforge::SecretResolverPort`

### Task 5: `[forge]` section in `NotstrapConfig`

**Crate**: `notstrap`
**File(s)**: `crates/notstrap/src/lib.rs`
**Run**: `cargo nextest run -p notstrap`

1. Write failing test:
   ```rust
   #[test]
   fn forge_section_absent_by_default() {
       let cfg: NotstrapConfig = toml::from_str(minimal_toml()).unwrap();
       assert!(cfg.forge.is_none());
   }

   #[test]
   fn forge_section_parses_when_present() {
       let toml = r#"
       [bootstrap]
       dotfiles_dir = "~/.notfiles"
       [forge]
       enabled = true
       config_path = "forge.toml"
       secret_ref = "gitea-token"
       on_error = "warn"
       "#;
       let cfg: NotstrapConfig = toml::from_str(toml).unwrap();
       let forge = cfg.forge.unwrap();
       assert!(forge.enabled);
       assert_eq!(forge.on_error, ForgeOnError::Warn);
   }
   ```

2. Add, mirroring the existing `tailscale: Option<TailscaleOptions>`
   pattern exactly:
   ```rust
   #[derive(Deserialize)]
   pub struct NotstrapConfig {
       pub bootstrap: BootstrapSection,
       pub tailscale: Option<TailscaleOptions>,
       #[serde(default)]
       pub hooks: Vec<notcore::HookSpec>,
       /// When present and enabled, notstrap ensures the configured forge
       /// repo exists before cloning dotfiles.
       pub forge: Option<ForgeSection>,
   }

   #[derive(Deserialize)]
   pub struct ForgeSection {
       #[serde(default)]
       pub enabled: bool,
       /// Path to a notforge ForgeConfig TOML file.
       pub config_path: PathBuf,
       /// Path to the local repo notforge should wire a remote onto.
       pub local_repo: PathBuf,
       #[serde(default = "default_on_error")]
       pub on_error: ForgeOnError,
   }

   #[derive(Deserialize, Debug, PartialEq, Eq, Default)]
   #[serde(rename_all = "lowercase")]
   pub enum ForgeOnError {
       #[default]
       Warn,
       Fail,
   }
   fn default_on_error() -> ForgeOnError { ForgeOnError::Warn }
   ```
   Deliberately a *primitive-shaped* section (paths + flags), not an
   embedded `notforge::ForgeConfig` — insulates `notstrap.toml` from
   `notforge`'s config schema, which the original design doc explicitly
   flagged as not yet stable.

3. Verify and commit: `feat(notstrap): add optional [forge] config section`

### Task 6: Wire the forge step into `run()`

**Crate**: `notstrap`
**File(s)**: `crates/notstrap/src/lib.rs`
**Run**: `cargo nextest run -p notstrap`

1. Write failing test (extends the existing `#[cfg(test)]` module style
   in `lib.rs`, or add to `tests/integration/tests/bootstrap.rs` per
   Task 7):
   ```rust
   #[test]
   fn forge_step_skipped_when_absent() {
       // opts with a config that has no [forge] section
       // run(opts) -> report has no "forge" step at all (not even Skipped)
   }

   #[test]
   fn forge_step_warns_and_continues_on_failure_by_default() {
       // [forge] enabled=true, but config_path points at a nonexistent file
       // report.add("forge", StepStatus::Failed(...)) but bootstrap continues to "clone dotfiles"
   }

   #[test]
   fn forge_step_fails_bootstrap_when_on_error_is_fail() {
       // same as above but on_error = "fail" -> run() returns early, no "clone dotfiles" step
   }
   ```

2. Insert step 3b (between Tailscale and clone) in `run()`:
   ```rust
   if let Some(ref forge) = cfg.forge {
       if forge.enabled {
           let outcome = run_forge_step(forge, &opts); // extracted fn, see below
           match (outcome, &forge.on_error) {
               (Ok(()), _) => report.add("forge", StepStatus::Ok),
               (Err(e), ForgeOnError::Warn) => {
                   report.add("forge", StepStatus::Failed(e.to_string()));
                   // continue — do not return early
               }
               (Err(e), ForgeOnError::Fail) => {
                   report.add("forge", StepStatus::Failed(e.to_string()));
                   return Ok(report);
               }
           }
       }
   }
   ```
   `run_forge_step` loads `notforge::load_config(&forge.config_path)`,
   builds `NotsecretsResolverAdapter` (needs a resolved
   `notsecrets::SecretResolver` in scope — resolve this before step 5's
   existing secrets block, or restructure so the secrets resolver is
   built once near the top and reused by both), then calls
   `notforge::resolve_auth` → `ensure_forge` (using
   `ExistingGiteaLifecycle`) → `ensure_repository` (using
   `GiteaHttpApi::new(base_url)`) → `ensure_git_remote` (using
   `CommandGitRemoteManager`) for each `RepoSpec` in the loaded config.

3. Verify: `cargo nextest run -p notstrap` all green;
   `cargo nextest run -p integration` (cross-crate tests) still green;
   `cargo clippy --workspace -- -D warnings` zero.

4. Commit: `feat(notstrap): wire optional forge step into bootstrap flow`

### Task 7: Cross-crate integration test

**Crate**: `tests/integration`
**File(s)**: `tests/integration/tests/bootstrap.rs` (extend) or new `tests/integration/tests/forge_bootstrap.rs`
**Run**: `cargo nextest run -p integration`

1. Add a test using fake `ForgeApi`/`ForgeLifecycle`/`GitRemoteManager`
   implementations (defined in the test file, not in `notforge`'s own
   `#[cfg(test)]` — cross-crate tests shouldn't reach into another
   crate's private test helpers) to drive a full `notstrap::run()` call
   with `[forge]` enabled, asserting:
   - no real network call is made (fakes only),
   - the report contains a `"forge"` step with `StepStatus::Ok`,
   - subsequent steps (clone/link/hooks) still run.

2. Add a companion test for the `on_error = "fail"` short-circuit path.

3. Verify and commit: `test(integration): cover forge-enabled bootstrap without live Gitea`

---

## Task Dependency Graph

```
T1 (ForgeLifecycle: Existing)  ─┐
T2 (GitRemoteManager: git CLI) ─┼─> T3 (orchestration fns in notforge::lib)
                                ─┘        │
                                          v
T4 (SecretResolverPort adapter) ────> T6 (wire forge step into run())
T5 ([forge] config section) ────────>    │
                                          v
                                    T7 (integration test)
```

## Suggested Execution Order

1. T1, T2 (independent, can parallelize) — the two missing adapters.
2. T3 — orchestration functions, depends on both adapters existing.
3. T4, T5 (independent, can parallelize) — notstrap-side plumbing.
4. T6 — the actual wiring, depends on T3, T4, T5 all landing.
5. T7 — proves the whole path end-to-end without a live Gitea.

## Risk Checklist

- [ ] T4's `ForgeSecretRef::Op` vs. `notsecrets`'s name-based
      `resolve(key)` may not line up 1:1 — resolve during
      implementation, not left ambiguous in the merged code.
- [ ] T6 changes control flow inside `notstrap::run()`, an already
      dense function with early-return-per-step-failure semantics.
      Mitigate: add the forge block as a single self-contained
      `if let` before touching anything else in the function, run the
      full existing `bootstrap.rs` integration suite after every change
      to confirm the non-forge path is byte-for-byte unaffected.
- [ ] `GiteaMode` variants other than `Existing` will parse
      successfully from TOML (the enum/deserializer already supports
      them) but fail at `ensure_available` time with a
      `NotforgeError::Command` — make sure this is a clear, actionable
      error message, not a silent no-op.
- [ ] New `notforge` dependency in `notstrap` — verify no dependency
      cycle (`notforge` must not gain a dependency on `notstrap`;
      confirmed clean today via `notforge/Cargo.toml`).
- [ ] Credential handling: `ForgeAuth` (containing a raw token/password)
      must never be included in `Report`/`StepStatus` error strings —
      audit `NotforgeError::Secret`/`Http` message construction in T1–T3
      to ensure no adapter interpolates the resolved secret value into
      an error message that later gets printed via `Report::print()`.
