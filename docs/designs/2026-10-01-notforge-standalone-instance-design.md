# Design: Standalone notforge — local Gitea instance, runner, and MCP control plane

## Goal

Promote `notfiles`' `notforge` crate into a standalone workspace at `~/dev/notforge`
that provisions a local Tailscale-reachable Gitea instance and its `act_runner`,
and exposes instance-lifecycle tooling over both a CLI and an MCP stdio server.

Extends `2026-07-08-on-demand-gitea-forge-design.md`, which stood the crate up
inside `notfiles`.

## Approved Approach

**Approach A** — native Gitea under `brew services` plus `act_runner` in a colima
Docker container. Scoped to **Phase 1 only**: a working instance and a proven
runner. Repo relay, crates.io publishing, and CI migration are follow-on phases
and are excluded here.

---

## Context Map

### Files to Modify (source repo — `notfiles`)

| File                                        | Purpose                      | Change                                                                                                                                                                                          |
| ------------------------------------------- | ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Cargo.toml`                                | workspace root               | remove `"crates/notforge"` from `members`; change `notforge = { path = ... }` to a version dep                                                                                                  |
| `Cargo.lock`                                | lockfile                     | remove the `notforge` package block; drop it from `notstrap`'s dep list                                                                                                                         |
| `taskit.toml`                               | task runner config           | remove notforge from the `crates` list, from the `notcore` propagation `dependents`, and from `publish_order`; delete the `notforge-ports` and `notforge-gitea` `[[protocol.surfaces]]` entries |
| `taskit-protocol.lock`                      | surface hashes               | delete the `notforge-ports` and `notforge-gitea` hash entries                                                                                                                                   |
| `Dockerfile`                                | e2e image                    | delete the notforge `COPY` (line 27) and its stub-loop entry (line 32) — the image already stubs notforge out as a "broken crate"                                                               |
| `crates/notstrap/Cargo.toml`                | consumer                     | `notforge` becomes a version dep                                                                                                                                                                |
| `crates/notstrap/src/forge.rs`              | `SecretResolverPort` adapter | compiles unchanged; superseded once notforge ships its own SOPS adapter                                                                                                                         |
| `docs/src/crates/notforge.md`               | crate chapter                | moves to the new repo                                                                                                                                                                           |
| `docs/src/architecture/overview.md`         | crate table                  | drop the notforge row; correct the false claim that "notgraph and notforge are siblings, not dependents"                                                                                        |
| `docs/src/README.md`, `docs/src/SUMMARY.md` | book index                   | drop notforge entries; regenerate `docs/dist` via `mdbook build`                                                                                                                                |
| `.claude/skills/working-with-notfiles/**`   | skill docs                   | 12 refs across `SKILL.md`, `config_patterns.json`, `documentation_index.json`                                                                                                                   |

`docs/designs/2026-07-08-on-demand-gitea-forge-design.md` stays in place as history.

### Dependencies

`notstrap` is the **only** production consumer. It uses **25 symbols, all via
fully-qualified `notforge::` paths with no `use` statements**
(`crates/notstrap/src/lib.rs:303-358`, `crates/notstrap/src/forge.rs`). The
extracted crate must expose all 25 at their current paths, or notstrap needs an
import-shuffling commit.

| Symbol group                                                                                                                                                                               | notstrap site                       |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------- |
| `notforge::config::{load_config, ForgeConfig, GiteaConfig, GiteaMode, ForgeAuthConfig, ForgeSecretRef, RepoSpec}`                                                                          | lib.rs:317, 337, 520, 582-596       |
| `notforge::error::NotforgeError`                                                                                                                                                           | forge.rs:4; lib.rs:505-575          |
| `notforge::ports::{SecretResolverPort, ForgeApi, ForgeLifecycle, GitRemoteManager, LocalRepository, RemoteRepository, ForgeVersion, LifecycleStatus, RemoteStatus, PushStatus, ForgeAuth}` | forge.rs:5; lib.rs:333-341, 502-566 |
| `notforge::{resolve_auth, ensure_forge, ensure_repository, ssh_remote_url, ensure_git_remote}`                                                                                             | lib.rs:344-354                      |
| `notforge::gitea::GiteaHttpApi`                                                                                                                                                            | lib.rs:318                          |
| `notforge::lifecycle::ExistingGiteaLifecycle`                                                                                                                                              | lib.rs:319                          |
| `notforge::git::CommandGitRemoteManager`                                                                                                                                                   | lib.rs:320                          |

`notcore` is the only cross-crate dependency: `notcore::config::load_toml_file`
at `crates/notforge/src/config.rs:173` — a 14-line generic TOML reader with no
`notcore`-specific types in its signature.

Zero `notforge` references exist in `notcore`, `notfiles`, `notsecrets`,
`nothooks`, `notnet`, `notgraph`, `notshell`, `tests/integration`, `xtask`,
`scripts`, or any workflow.

### Test Coverage

23 existing tests, all offline. **Zero `#[ignore]`, zero env-var gates, zero
`required-features`**, and no `[features]` table in the manifest.

| Location                  | Count | Covers                                                                                                                                                         |
| ------------------------- | ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/lib.rs` inline       | 7     | `VERSION`, `resolve_auth` precedence/fallback/error, `ensure_repository` create + idempotence, `ensure_git_remote` delegation                                  |
| `src/lifecycle.rs` inline | 3     | `Existing` verified, error propagation, non-`Existing` mode rejected                                                                                           |
| `src/git.rs` inline       | 4     | `remote_url` absent, `set_remote` add/unchanged/update — real `git init` in a `TempDir`                                                                        |
| `tests/api.rs`            | 4     | `miette::Diagnostic` derive, `VmSystemd` config parse, `Existing` string-form parse with serde defaults, **object-safety regression guard** for all four ports |
| `tests/gitea.rs`          | 4     | request construction, auth header, status handling — all via `FakeTransport`                                                                                   |

Gaps this phase must close: no coverage for `LocalProcess` lifecycle, runner
registration, or template rendering.

### Reference Patterns

| Pattern                                                      | Source to follow                                                  |
| ------------------------------------------------------------ | ----------------------------------------------------------------- |
| clap derive types in the lib, `anyhow` at the boundary       | `crates/notfiles/src/cli.rs`, `crates/notfiles/src/main.rs:16-46` |
| composition root constructing all adapters in one `run_*` fn | `crates/notstrap/src/lib.rs:303-330` (`run_forge_step`)           |
| shelling out with discrete `.arg()` and a `// SAFETY:` note  | `crates/notstrap/src/repo.rs` (`clone_if_missing`)                |
| non-zero exit surfacing stderr                               | `crates/notnet/src/installer.rs:79-90`                            |
| injectable HTTP transport seam                               | `crates/notforge/src/gitea.rs:53-58` (`GiteaTransport`)           |
| `deny.toml`, copied verbatim                                 | `notfiles/deny.toml`                                              |

### Risk

- [ ] **Public API change: yes.** `ForgeApi` gains two methods; `ForgeConfig` and `LocalProcessConfig` gain fields; `HttpMethod` gains `Patch`/`Delete`. notstrap's four fakes are in-file and must be updated in the same commit.
- [ ] **Serialization:** `notforge.toml` gains an optional `[runner]` table. Additive; no existing config breaks.
- [ ] **Cross-repo dependency flip:** `notfiles` gains an external dep on published `notforge`, so release ordering becomes manual. `notfiles/.github/workflows/release.yml:73-131` derives affected crates by reverse-dep walk; once notforge is external its version is no longer auto-coordinated.
- [ ] **`taskit protocol drift` fails** in notfiles until the two `[[protocol.surfaces]]` entries and their lock hashes are removed.
- [ ] `Dockerfile` never compiled notforge, so extraction removes it from the e2e image without losing coverage.

---

## Crate Ownership

New standalone workspace `~/dev/notforge`: edition 2024, `rust-version = "1.87.0"`,
`license = "MIT OR Apache-2.0"`, its own `taskit.toml` + `xtask`, `deny.toml` copied
verbatim from notfiles.

- **Owner crate**: `notforge` — owns forge lifecycle, Gitea API operations, repository provisioning, git remote wiring, runner control, and config templating.
- **Second crate**: `notforge-mcp` — owns MCP protocol translation only.
- **Affected crate**: `notfiles`'s `notstrap` — becomes a downstream version consumer.

`notforge` is its own repo because it is a forge tool, not a dotfiles tool, and its
consumers are external. `notforge-mcp` is separate because `rmcp` pulls a tokio/async
tree the provisioning CLI never uses — mirroring the existing `braid` / `braid-mcp`
split in this workspace.

Two crates, not more.

---

## Secret Management

Modelled directly on `minibox/sops.just`, which materializes a SOPS-encrypted
secret into a `0600` file before an external tool consumes it.

**Repository layout** (new files in `~/dev/notforge`):

| File                      | Purpose                                                                            |
| ------------------------- | ---------------------------------------------------------------------------------- |
| `.sops.yaml`              | `creation_rules` scoped to `secrets/.*\.sops\.ya?ml$`, pinned to the age recipient |
| `secrets/forge.sops.yaml` | flat YAML keys, committed encrypted: `gitea_admin_password`, `gitea_api_token`     |
| `sops.just`               | `auth-materialize` recipe, mirroring `minibox/sops.just`                           |

`sops.just` follows the minibox recipe shape: `set shell := ["bash", "-cu"]`,
`set -euo pipefail`, prerequisite checks that fail with actionable paths, decrypt
into `$(mktemp)`, `chmod 600`, atomic `mv`, `chmod 600` on the target, and a final
`[OK]` line naming the file written. Age key resolution honors
`SOPS_AGE_KEY_FILE`, defaulting to `$HOME/.config/sops/age/keys.txt`.

**Resolution inside notforge** goes through `SopsSecretResolver`, which shells
`sops --decrypt --extract '["<key>"]' <file>` through `ProcessExec`. Per-key
extraction rather than whole-file decryption, so no unrelated secret is ever held in
memory and a single key can be requested without parsing a full document.

**Bootstrap ordering.** `notforge auth bootstrap` creates the admin user from
`gitea_admin_password`; `notforge auth mint-token` mints the API token from
`POST /users/{owner}/tokens`; both values are then captured into
`secrets/forge.sops.yaml`. `init` refuses to overwrite an existing `app.ini`, so the
bootstrap is once-only and every later run resolves from the encrypted file.

**`notforge auth check`** reports whether each configured reference resolves,
printing only the variant, key name, and file path — never a secret value. It is the
MCP-exposed form of the same check.

---

## Public API

### Modules

```rust
// crates/notforge/src/lib.rs
pub mod auth;      // new — SopsSecretResolver, EnvSecretResolver, resolve_secret
pub mod config;
pub mod error;
pub mod git;
pub mod gitea;
pub mod lifecycle;
pub mod ports;
pub mod proc;      // new — ProcessExec port, CommandRequest/CommandOutput, StdProcessExec
pub mod runner;    // new — runner registration and control domain logic
pub mod storage;   // new — ConfigFile port, FsConfigFile
pub mod template;  // new — placeholder rendering
```

### Traits

```rust
// crates/notforge/src/ports.rs — extended, not replaced

pub trait ForgeApi {
    fn version(&self) -> Result<ForgeVersion, NotforgeError>;

    fn repo(&self, owner: &str, name: &str) -> Result<Option<RemoteRepository>, NotforgeError>;

    fn create_repo(&self, spec: &RepoSpec, auth: &ForgeAuth) -> Result<RemoteRepository, NotforgeError>;

    /// Mint an instance-wide Actions runner registration token.
    fn runner_registration_token(&self, auth: &ForgeAuth) -> Result<String, NotforgeError>;

    /// List registered Actions runners.
    fn runners(&self, auth: &ForgeAuth) -> Result<Vec<RunnerInfo>, NotforgeError>;
}

// crates/notforge/src/proc.rs — new port

pub trait ProcessExec {
    /// Spawn `request.program` with `request.args`, returning exit status and captured output.
    fn run(&self, request: &CommandRequest) -> Result<CommandOutput, NotforgeError>;
}

// crates/notforge/src/storage.rs — new port

pub trait ConfigFile {
    /// Write `contents` to `path`, creating parent directories as needed.
    fn write(&self, path: &Path, contents: &str) -> Result<(), NotforgeError>;

    /// Read `path` to a string.
    fn read(&self, path: &Path) -> Result<String, NotforgeError>;
}
```

`ForgeLifecycle` is deliberately **unchanged** — see Decision 3.

### Types

```rust
// crates/notforge/src/ports.rs

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerInfo {
    pub id: u64,
    pub name: String,
    pub labels: Vec<String>,
    pub online: bool,
}

// crates/notforge/src/proc.rs

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRequest {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn success(&self) -> bool;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct StdProcessExec;

// crates/notforge/src/auth.rs — new adapters

#[derive(Debug, Default, Clone, Copy)]
pub struct SopsSecretResolver {
    /// Encrypted secret file, defaulting to `secrets/forge.sops.yaml`.
    pub file: PathBuf,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EnvSecretResolver;

// crates/notforge/src/storage.rs

#[derive(Debug, Default, Clone, Copy)]
pub struct FsConfigFile;

// crates/notforge/src/runner.rs

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerRegistration {
    pub token: String,
    pub config_path: PathBuf,
}

// crates/notforge/src/config.rs — additions

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgeConfig {
    pub gitea: GiteaConfig,
    #[serde(default)]
    pub runner: RunnerConfig,
    #[serde(default)]
    pub repositories: Vec<RepoSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnerConfig {
    /// Container runtime executable used for the runner container.
    pub runtime: String,
    /// Runner container image reference.
    pub image: String,
    /// Container name for the runner.
    pub container_name: String,
    /// Runner labels advertised to Gitea.
    pub labels: Vec<String>,
    /// Runner poll interval, as a Go duration string.
    #[serde(default = "RunnerConfig::default_interval")]
    pub interval: String,
}

impl RunnerConfig {
    pub fn default_interval() -> String;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalProcessConfig {
    pub binary: String,
    pub work_dir: String,
    pub config_path: String,
    /// Homebrew service label, when the process is managed by `brew services`.
    #[serde(default)]
    pub service: Option<String>,
}

// crates/notforge/src/config.rs — ForgeSecretRef gains a variant.
// The enum is `#[serde(tag = "source", rename_all = "lowercase")]` with a
// hand-written kind table, mirroring GiteaMode's existing shape.

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForgeSecretRef {
    /// Resolve from an environment variable.
    Env { key: String },
    /// Resolve from a 1Password URI. Not implemented by notforge.
    Op { uri: String },
    /// Resolve a single key from a SOPS-encrypted YAML file.
    Sops { key: String, file: PathBuf },
}

// crates/notforge/src/gitea.rs — additions

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Patch,
    Delete,
}

// crates/notforge/src/lifecycle.rs — additions

#[derive(Debug, Clone, Copy)]
pub struct LocalProcessLifecycle<'a> {
    pub exec: &'a dyn ProcessExec,
}

impl<'a> LocalProcessLifecycle<'a> {
    pub fn new(exec: &'a dyn ProcessExec) -> Self;
}
```

### Functions

```rust
// crates/notforge/src/template.rs — new

/// Replace `{{key}}` placeholders in `template` with values from `vars`.
pub fn render(template: &str, vars: &[(&str, &str)]) -> String;

// crates/notforge/src/proc.rs — new

// crates/notforge/src/auth.rs — new

/// Resolve a secret reference, dispatching on its `ForgeSecretRef` variant.
pub fn resolve_secret(
    secret: &ForgeSecretRef,
    env: &dyn SecretResolverPort,
    sops: &dyn SecretResolverPort,
) -> Result<String, NotforgeError>;

// crates/notforge/src/runner.rs — new

/// Render the local-process Gitea `app.ini` from the bundled template.
pub fn render_app_ini(config: &ForgeConfig) -> Result<String, NotforgeError>;

/// Mint a runner registration token and write the runner config file.
pub fn register_runner(
    config: &ForgeConfig,
    api: &dyn ForgeApi,
    auth: &ForgeAuth,
    fs: &dyn ConfigFile,
) -> Result<RunnerRegistration, NotforgeError>;

/// Start the runner container.
pub fn start_runner(config: &ForgeConfig, exec: &dyn ProcessExec) -> Result<(), NotforgeError>;

/// Stop the runner container.
pub fn stop_runner(config: &ForgeConfig, exec: &dyn ProcessExec) -> Result<(), NotforgeError>;

// crates/notforge/src/lifecycle.rs — addition

/// Stop the configured local-process Gitea backend.
pub fn stop_forge(
    config: &ForgeConfig,
    exec: &dyn ProcessExec,
) -> Result<LifecycleStatus, NotforgeError>;
```

All existing public signatures — `resolve_auth`, `ensure_forge`, `ensure_repository`,
`ssh_remote_url`, `ensure_git_remote`, `load_config`, `GiteaHttpApi::new`,
`GiteaHttpApi::with_transport`, `VERSION` — are **unchanged**.

### Binary: `notforge`

```rust
// crates/notforge/src/cli.rs — new, in the lib so it stays unit-testable,
// mirroring crates/notfiles/src/cli.rs

#[derive(Parser, Debug)]
#[command(name = "notforge", version, about = "Provision a local Gitea forge")]
pub struct Cli {
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    #[arg(long, global = true)]
    pub verbose: bool,
    #[arg(long, short, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Init {
        #[arg(long)]
        force: bool,
    },
    Up,
    Down,
    Status,
    Login {
        #[arg(long)]
        name: String,
    },
    Ensure {
        #[arg(long)]
        local_repo: Option<PathBuf>,
    },
    Auth {
        #[command(subcommand)]
        action: AuthCommand,
    },
    Runner {
        #[command(subcommand)]
        action: RunnerCommand,
    },
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    /// Create the instance admin user, resolving its password from config.
    Bootstrap,
    /// Mint an API token for the configured owner.
    MintToken,
    /// Report which secret references resolve, without printing any value.
    Check,
}

#[derive(Subcommand, Debug)]
pub enum RunnerCommand {
    Register,
    Start,
    Stop,
    Status,
}
```

`main.rs` returns `anyhow::Result<()>`, matching `notfiles` and `notstrap`.
`NotforgeError` stays internal and keeps its `miette::Diagnostic` derive.

### Binary: `notforge-mcp`

`rmcp` 3.1 with `server` + `transport-io`, stdio transport, matching `personal-mcp`'s
SDK version.

| Tool               | Mutating | Backing port                                                    |
| ------------------ | -------- | --------------------------------------------------------------- |
| `instance_status`  | no       | `ForgeApi::version`                                             |
| `instance_up`      | yes      | `ForgeLifecycle`, `ProcessExec`                                 |
| `instance_down`    | yes      | `ProcessExec`                                                   |
| `auth_status`      | no       | `SecretResolverPort` — reports resolvability only, never values |
| `runner_register`  | yes      | `ForgeApi`, `ConfigFile`                                        |
| `runner_start`     | yes      | `ProcessExec`                                                   |
| `runner_stop`      | yes      | `ProcessExec`                                                   |
| `runner_status`    | no       | `ForgeApi::runners`                                             |
| `repo_ensure`      | yes      | `ForgeApi`                                                      |
| `remote_sync`      | yes      | `GitRemoteManager`                                              |
| `tea_login_status` | no       | `ProcessExec`                                                   |

Resources: `notforge://instance/config`, `notforge://instance/runners`.
Prompt: `provision-instance`.

Mutating tools are gated on `NOTFORGE_MCP_ALLOW_MUTATIONS=true`, mirroring
`personal-mcp`'s `PERSONAL_MCP_ALLOW_MUTATIONS`. Output is truncated at 8 KB,
matching `personal-mcp`.

Daily repository, pull-request, issue, and workflow-run operations are **not** in
this server: official `gitea-mcp` already covers them against spec `2026-07-28` and is
registered alongside as a second MCP server.

---

## Data Flow

1. **Config** — `notforge.toml` is read from disk via `config::load_config` into `ForgeConfig`.
2. **Auth** — `ForgeAuthConfig` refs resolve to a `ForgeAuth` via `resolve_secret`: `ForgeSecretRef::Env` to `EnvSecretResolver`, `ForgeSecretRef::Sops` to `SopsSecretResolver`, `ForgeSecretRef::Op` rejected as unsupported.
3. **Template** — `render_app_ini` substitutes `ForgeConfig` values into `templates/app.ini.tmpl` via `template::render`.
4. **Lifecycle** — `LocalProcessLifecycle::ensure_available` shells `brew services start gitea` through `ProcessExec`; `stop_forge` shells `brew services stop gitea`.
5. **Runner** — `register_runner` mints a token via `ForgeApi::runner_registration_token`, renders `templates/runner-config.yaml.tmpl`, and writes it through `ConfigFile`.
6. **Runner container** — `start_runner` shells `docker run -d` with the mounted Docker socket and the configured labels through `ProcessExec`.
7. **Provisioning** — `ensure_repository` reads `ForgeConfig::repositories` and calls `ForgeApi::create_repo`; `ensure_git_remote` calls `GitRemoteManager::set_remote`.
8. **MCP** — an `rmcp` tool handler adapts one tool call into the same domain functions the CLI calls. The MCP server holds no domain logic of its own.

---

## Hexagonal Boundaries

| Port (trait)         | Location            | Adapters                                          |
| -------------------- | ------------------- | ------------------------------------------------- |
| `ForgeApi`           | `notforge::ports`   | `GiteaHttpApi<T: GiteaTransport>`                 |
| `GiteaTransport`     | `notforge::gitea`   | `UreqGiteaTransport`                              |
| `ForgeLifecycle`     | `notforge::ports`   | `ExistingGiteaLifecycle`, `LocalProcessLifecycle` |
| `GitRemoteManager`   | `notforge::ports`   | `CommandGitRemoteManager`                         |
| `SecretResolverPort` | `notforge::ports`   | `SopsSecretResolver`, `EnvSecretResolver`         |
| `ProcessExec`        | `notforge::proc`    | `StdProcessExec`                                  |
| `ConfigFile`         | `notforge::storage` | `FsConfigFile`                                    |

Every external effect — HTTP, subprocess, filesystem, SOPS decryption — sits behind a trait.
`notforge-mcp` and the `notforge` binary are both composition roots and contain no
domain logic.

---

## Decisions

1. **Secrets come from SOPS + age, not 1Password.** Auth is in scope. The pattern follows `minibox/sops.just`: a committed encrypted `secrets/forge.sops.yaml` with `.sops.yaml` creation rules pinned to the age recipient, per-key extraction via `sops --decrypt --extract`, and materialization at `0600` through `mktemp` plus atomic `mv`. `ForgeSecretRef` gains a `Sops { key, file }` variant.

   This also resolves the gap the earlier draft flagged: the only `SecretResolverPort` implementation in existence — `notstrap/src/forge.rs:28-43` — returns a hard error for `Op`, because `notsecrets::SecretResolver` can only resolve a name bound in `notfiles`' `notsecrets.toml`, never an ad hoc `op://` URI. `ForgeSecretRef::Op` is retained so existing configs still parse, and `resolve_secret` rejects it with the same guidance. `notsecrets` is not adopted; depending on it would recreate the coupling this extraction exists to remove.

   **argv exposure.** `gitea admin user create --password` and `tea login add --token` both take secrets as command-line arguments, which are briefly visible in process listings. Both are invoked with discrete `.arg()` calls and no shell, matching the audited `notstrap::repo::clone_if_missing` pattern. `gitea admin user create --random-password` is the documented alternative for the bootstrap if argv exposure is unacceptable; it trades a stored password for a one-time printed one.

2. **The hand-rolled HTTP client is kept.** `GiteaHttpApi` + `GiteaTransport` stays the adapter. Phase 1 needs about six endpoints (`version`, `repo`, `create_repo`, `runner_registration_token`, `runners`, and whatever `runner start` probes). Adopting `gitea-client` would import reqwest and tokio into a CLI that stays synchronous, and would break `GiteaHttpApi::new` at `notstrap/src/lib.rs:318`. `HttpMethod` gains `Patch` and `Delete` for completeness, not because Phase 1 calls them.

3. **`ForgeLifecycle` is not extended.** Adding a `stop` method would break `notstrap`'s `FakeLifecycle` at `crates/notstrap/src/lib.rs:533`, and a defaulted `stop` returning success would be a lie. `stop_forge` is a free function over `ProcessExec` instead, leaving the trait and its object-safety guard untouched.

4. **`template::render` is hand-rolled.** `{{key}}` substitution over a `&[(&str, &str)]` pair list. No Tera, no MiniJinja, no serde-based templating — the templates are two files with a handful of variables.

5. **Templates ship as `templates/*.tmpl` in the repo, not embedded.** Read at runtime from the configured path, so a template can be inspected and corrected without rebuilding the binary.

---

## Out of Scope

- **Phase 2** — notforge as a relay between local repos and GitHub; crates.io publishing from Gitea Actions.
- **Phase 3** — CI migration. ~85 repos carry `.github/workflows`; only 4 carry `.gitea/workflows`. Gitea has no macOS runner, so macOS jobs remain GitHub-side.
- **Phase 4** — flipping source-of-truth to Gitea.
- **Push-mirror auth** — deferred. The single-repo write-only SSH deploy key is the leading candidate over a PAT stored in `gitea.db`.
- **Repository migration** — fresh instance, zero repos migrated. `notforge ensure` is the seam it grows from.
- **Crate publish policy** — read from each crate's existing `publish` field, never invented. 169 of 827 workspace manifests already set `publish = false`.
- **PR / issue / workflow-run MCP tools** — delegated to official `gitea-mcp`.
- **`notforge destroy`** — no destructive command exists. `down` stops services and never deletes `gitea.db` or the repo root.

---

## Risk

- [ ] **Breaking API changes: partial.** All public additions are backward compatible, but `ForgeApi` gains two methods, so every implementor must add them. notstrap's four fakes (`crates/notstrap/src/lib.rs:504, 533, 543, 571`) and the object-safety guard in `tests/api.rs:106` need updating in the same commit.
- [ ] **New external dependencies:**
  - `clap` 4 (derive) and `clap_complete` 4 — for the `notforge` binary, already used workspace-wide.
  - `anyhow` 1 — CLI boundary only, matching `notfiles` and `notstrap`.
  - `rmcp` 3.1 (`server`, `transport-io`) and `tokio` 1 — `notforge-mcp` only, kept out of the CLI tree.
  - No HTTP client added; `ureq` 2 carries over unchanged.
- [ ] **Feature flag required: no.** The crate has no `[features]` table today and this design keeps it that way; `notforge-mcp` isolation is achieved by crate separation instead.
- [ ] **Serialization change: additive.** `[runner]`, `LocalProcessConfig::service`, and the new `ForgeSecretRef::Sops` variant are all additive; existing `notforge.toml` files keep parsing.
- [ ] **New external runtime dependencies: `sops` and `age`.** Both already resolve from the Nix profile (`~/.nix-profile/bin`), and the age private key already exists at `~/.config/sops/age/keys.txt`. Neither is a Cargo dependency; both are reached through `ProcessExec`.
- [ ] **Release ordering.** notforge must be published before notfiles can bump to it. `.github/workflows/release.yml:73-131` in notfiles will no longer auto-coordinate the version.
- [ ] **Protocol drift.** notfiles `taskit protocol drift` fails until both `[[protocol.surfaces]]` entries and their lock hashes are removed.
