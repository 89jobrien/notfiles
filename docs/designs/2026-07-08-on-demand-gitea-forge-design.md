# Design: On-demand Gitea Forge

## Goal

Implement a dedicated forge component for `notfiles` that can ensure a configured Gitea backend is available on demand, create repositories through the Gitea API, and wire local git remotes for push workflows.

## Approved Approach

Use a new dedicated `notforge` crate and CLI that supports both VM-hosted and local Gitea modes via config, with `notstrap` integration deferred until the `notforge` API is tested.

## Context Map

### Files to Modify

- `Cargo.toml` — add `crates/notforge` to the workspace and shared dependencies needed by the new crate.
- `crates/notforge/Cargo.toml` — define the new library and binary crate.
- `crates/notforge/src/lib.rs` — expose the public `notforge` modules and orchestration functions.
- `crates/notforge/src/config.rs` — own forge config, backend mode, repository specs, and secret reference types.
- `crates/notforge/src/error.rs` — own forge-specific errors.
- `crates/notforge/src/ports.rs` — define hexagonal ports for forge API, lifecycle, git remote management, and secret resolution.
- `crates/notforge/src/gitea.rs` — implement Gitea API request/response types and adapter boundaries.
- `crates/notforge/src/lifecycle.rs` — implement command planning for local and VM Gitea lifecycle backends.
- `crates/notforge/src/git.rs` — implement remote URL derivation and git remote operations behind a port.
- `crates/notforge/src/main.rs` — expose the `notforge` CLI.
- `crates/notforge/tests/integration.rs` — verify config parsing, command planning, API request construction, and git remote behavior with fakes/temp repos.
- `crates/notstrap/src/lib.rs` — later optional integration point for `[forge]`, after the `notforge` library is stable.
- `crates/notstrap/Cargo.toml` — later dependency on `notforge`, only when integration is implemented.
- `README.md` — document the separate config-payload repo and the future `notforge` flow.
- `AGENTS.md` — update workspace architecture notes to include `notforge`.

### Dependencies

- `notforge` depends inward on `notcore` for shared reporting conventions if needed.
- `notforge` may depend on `notsecrets` for credential resolution, but external secret providers stay behind a `SecretResolverPort`.
- `notstrap` may later depend on `notforge`, but `notforge` must not depend on `notstrap`.
- `notnet` remains responsible for network presence; `notforge` may compose with it later but does not replace it.

### Test Coverage

- Existing `tests/integration/tests/bootstrap.rs` covers `notstrap` bootstrap flow and should remain unchanged until `[forge]` integration is added.
- Existing `crates/notfiles/tests/integration.rs` covers package linking and should remain unaffected.
- New `crates/notforge/tests/integration.rs` should cover the new crate without requiring a live Gitea instance.

### Risk

- Public API addition: yes — new crate and binary.
- Existing public API breakage: no — initial design adds `notforge` without changing `notstrap` behavior.
- External I/O risk: yes — HTTP, process, SSH/systemd, and git operations must stay behind ports.
- Secret handling risk: yes — credentials must not be printed, logged, or stored in config.

## Crate Ownership

- **Owner crate**: `notforge` — owns forge lifecycle, Gitea API operations, repository provisioning, and git remote wiring.
- **Affected crates**:
  - `notcore` — optional shared report/status types only.
  - `notsecrets` — credential resolution through a port adapter.
  - `notstrap` — future caller when `[forge]` config is enabled.
  - `notnet` — future composition for tailnet readiness before VM forge operations.

`notforge` is a new crate because forge lifecycle and repository provisioning are not dotfile linking (`notfiles`), hook execution (`nothooks`), network presence (`notnet`), or full-machine orchestration (`notstrap`). Its single responsibility is managing forge availability and repository operations.

## Public API

### Traits

```rust
pub trait ForgeApi {
    fn version(&self) -> Result<ForgeVersion, NotforgeError>;
    fn repo(&self, owner: &str, name: &str) -> Result<Option<RemoteRepository>, NotforgeError>;
    fn create_repo(&self, spec: &RepoSpec, auth: &ForgeAuth) -> Result<RemoteRepository, NotforgeError>;
}
```

```rust
pub trait ForgeLifecycle {
    fn ensure_available(&self, config: &ForgeConfig) -> Result<LifecycleStatus, NotforgeError>;
}
```

```rust
pub trait GitRemoteManager {
    fn remote_url(&self, repo: &LocalRepository, remote_name: &str) -> Result<Option<String>, NotforgeError>;
    fn set_remote(&self, repo: &LocalRepository, remote_name: &str, url: &str) -> Result<RemoteStatus, NotforgeError>;
    fn push(&self, repo: &LocalRepository, remote_name: &str, branch: &str, set_upstream: bool) -> Result<PushStatus, NotforgeError>;
}
```

```rust
pub trait SecretResolverPort {
    fn resolve(&self, secret: &ForgeSecretRef) -> Result<String, NotforgeError>;
}
```

### Types

```rust
pub struct ForgeConfig {
    pub gitea: GiteaConfig,
    pub repositories: Vec<RepoSpec>,
}
```

```rust
pub struct GiteaConfig {
    pub base_url: String,
    pub owner: String,
    pub ssh_host: String,
    pub ssh_port: u16,
    pub mode: GiteaMode,
    pub auth: ForgeAuthConfig,
}
```

```rust
pub enum GiteaMode {
    LocalProcess(LocalProcessConfig),
    LocalContainer(LocalContainerConfig),
    VmSystemd(VmSystemdConfig),
    Existing,
}
```

```rust
pub struct LocalProcessConfig {
    pub binary: String,
    pub work_dir: String,
    pub config_path: String,
}
```

```rust
pub struct LocalContainerConfig {
    pub runtime: String,
    pub container_name: String,
    pub image: String,
    pub data_dir: String,
}
```

```rust
pub struct VmSystemdConfig {
    pub ssh_user: String,
    pub ssh_host: String,
    pub service_name: String,
}
```

```rust
pub struct ForgeAuthConfig {
    pub token: Option<ForgeSecretRef>,
    pub username: Option<ForgeSecretRef>,
    pub password: Option<ForgeSecretRef>,
}
```

```rust
pub enum ForgeSecretRef {
    Env { key: String },
    Op { uri: String },
}
```

```rust
pub enum ForgeAuth {
    Token { token: String },
    Basic { username: String, password: String },
}
```

```rust
pub struct RepoSpec {
    pub owner: String,
    pub name: String,
    pub private: bool,
    pub description: Option<String>,
    pub remote_name: String,
}
```

```rust
pub struct RemoteRepository {
    pub owner: String,
    pub name: String,
    pub clone_url: String,
    pub ssh_url: String,
}
```

```rust
pub struct ForgeVersion {
    pub version: String,
}
```

```rust
pub struct LocalRepository {
    pub path: std::path::PathBuf,
}
```

```rust
pub enum LifecycleStatus {
    AlreadyAvailable,
    Started,
    Verified,
}
```

```rust
pub enum RemoteStatus {
    Added,
    Updated,
    Unchanged,
}
```

```rust
pub enum PushStatus {
    Pushed,
    AlreadyUpToDate,
}
```

```rust
pub enum NotforgeError {
    Config(String),
    Secret(String),
    Http(String),
    Command(String),
    Git(String),
    Repository(String),
}
```

### Functions

```rust
pub fn load_config(path: &std::path::Path) -> Result<ForgeConfig, NotforgeError>;
```

```rust
pub fn resolve_auth(
    config: &ForgeAuthConfig,
    secrets: &dyn SecretResolverPort,
) -> Result<ForgeAuth, NotforgeError>;
```

```rust
pub fn ensure_forge(
    config: &ForgeConfig,
    lifecycle: &dyn ForgeLifecycle,
) -> Result<LifecycleStatus, NotforgeError>;
```

```rust
pub fn ensure_repository(
    api: &dyn ForgeApi,
    auth: &ForgeAuth,
    spec: &RepoSpec,
) -> Result<RemoteRepository, NotforgeError>;
```

```rust
pub fn ssh_remote_url(config: &GiteaConfig, repo: &RepoSpec) -> String;
```

```rust
pub fn ensure_git_remote(
    manager: &dyn GitRemoteManager,
    local: &LocalRepository,
    repo: &RepoSpec,
    remote_url: &str,
) -> Result<RemoteStatus, NotforgeError>;
```

## Data Flow

1. Source: `notforge` reads a forge config file and repository spec from disk or CLI arguments.
2. Transform: `notforge` resolves secrets through `SecretResolverPort` and maps config into `ForgeAuth`, `GiteaMode`, and `RepoSpec`.
3. Transform: `ForgeLifecycle` verifies or starts the configured Gitea backend.
4. Transform: `ForgeApi` checks whether the remote repository exists and creates it if missing.
5. Sink: `GitRemoteManager` sets the local git remote to the derived SSH URL.
6. Sink: `GitRemoteManager` pushes the requested branch when explicitly invoked.

## Hexagonal Boundaries

- **Port**: `ForgeApi` in `notforge::ports` abstracts Gitea HTTP operations.
- **Adapter**: `GiteaHttpApi` in `notforge::gitea` implements `ForgeApi`.
- **Port**: `ForgeLifecycle` in `notforge::ports` abstracts local/VM service startup.
- **Adapter**: `LocalProcessLifecycle`, `LocalContainerLifecycle`, and `VmSystemdLifecycle` in `notforge::lifecycle` implement `ForgeLifecycle`.
- **Port**: `GitRemoteManager` in `notforge::ports` abstracts git remote and push operations.
- **Adapter**: `CommandGitRemoteManager` in `notforge::git` implements `GitRemoteManager`.
- **Port**: `SecretResolverPort` in `notforge::ports` abstracts credential resolution.
- **Adapter**: `NotsecretsResolverAdapter` in `notforge::secrets` resolves `ForgeSecretRef` through `notsecrets`.

## Integration Points

- Workspace integration: add `crates/notforge` as a workspace member.
- CLI integration: expose a `notforge` binary rather than adding forge subcommands to `notfiles`.
- Bootstrap integration: add optional `[forge]` support to `notstrap` only after `notforge` is tested.
- Network integration: VM mode can be composed after `notnet::ensure_connected`, but `notforge` does not join Tailscale itself in the initial design.
- Current migration integration: use `notforge repo ensure` later to create `joe/notfiles-config`, then push the existing local commits.

## Out of Scope

- Full Gitea installation from scratch on a remote VM.
- Managing Gitea users, SSH keys, runners, or Actions configuration.
- Implementing GitHub/GitLab providers.
- Replacing `notfiles` link/status/adopt behavior.
- Moving personal config payloads back into the `notfiles` tool repo.
- Storing credentials in config files.

## Risk

- [ ] Breaking API changes: no — initial `notforge` is additive.
- [ ] New external dependency: yes — an HTTP client is likely required for the Gitea adapter and must stay behind `ForgeApi`.
- [ ] Feature flag required: no for the crate; optional `notstrap` integration can be gated by config presence.
- [ ] Circular dependency risk: avoid by keeping `notforge` independent of `notstrap`.
- [ ] Secret exposure risk: high unless all auth values are resolved in memory and never printed.
- [ ] Command execution risk: medium; all process, container, SSH, systemd, and git calls must be adapter-level and argument-based.
