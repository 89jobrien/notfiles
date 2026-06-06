# notsecrets Multi-Provider Secret Resolution

**Date:** 2026-06-06
**Crate:** `notsecrets`
**Status:** Approved design, pending implementation plan

## Goal

Extend notsecrets with a general-purpose secret/env resolution system alongside
the existing age identity system. Multiple backends resolve arbitrary secrets
(API keys, tokens, env vars) via a typed provider chain with explicit bindings.

## Non-Goals

- Replacing the existing `IdentitySource` / age identity system
- Runtime hot-reloading of config
- Secret rotation or lease management
- Encrypting secrets (write path) -- read-only resolution

## Architecture

### Two independent port systems in one crate

```
notsecrets
  +-- Age identity system (existing, untouched)
  |     IdentitySource trait -> BitwardenSource, FileSource, PromptSource, YubikeySource
  |     resolve_identities() -> Vec<Box<dyn Identity>>
  |
  +-- Secret resolution system (new)
        SecretSource trait -> 11 provider adapters
        SecretResolver -> config-driven resolution with bindings
```

### Core Traits

```rust
/// Port: resolve a single secret by key name.
pub trait SecretSource {
    fn name(&self) -> &str;
    fn provider(&self) -> Provider;
    fn resolve(&self, key: &str) -> Result<Option<String>, SecretsError>;

    /// Resolve using a provider-specific typed reference.
    /// Default: ignores ref, falls back to resolve(key).
    fn resolve_ref(
        &self,
        key: &str,
        secret_ref: &SecretRef,
    ) -> Result<Option<String>, SecretsError> {
        let _ = secret_ref;
        self.resolve(key)
    }
}

/// Extended port: sources that can enumerate all available secrets.
pub trait EnumerableSecretSource: SecretSource {
    fn resolve_all(&self) -> Result<HashMap<String, String>, SecretsError>;
}
```

ISP rationale: not every backend supports enumeration. `OpSource` and `GsmSource`
require explicit refs for lookup -- they cannot enumerate a vault without extra
config. Forcing `resolve_all()` on every impl would produce incomplete maps.

### Type System

#### Provider enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Env,
    Op,
    Dotenvx,
    Sops,
    Gsm,
    Nuenv,
    Direnv,
    Mise,
    Bitwarden,
    Vault,
    Dotenvy,
}
```

#### Provider config (tagged enum)

```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ProviderConfig {
    Env,
    Op { account: String },
    Dotenvx { env_file: PathBuf },
    Sops { file: PathBuf },
    Gsm { project: String },
    Nuenv,
    Direnv,
    Mise,
    Bitwarden { server_url: Option<String> },
    Vault { addr: String, mount: Option<String> },
    Dotenvy { path: PathBuf },
}
```

#### Secret references (tagged enum)

```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum SecretRef {
    Env,
    Op { uri: String },
    Dotenvx { key: Option<String> },
    Sops { key: Option<String> },
    Gsm { path: String },
    Nuenv { key: Option<String> },
    Direnv { key: Option<String> },
    Mise { key: Option<String> },
    Bitwarden { item: String, field: Option<String> },
    Vault { path: String, field: Option<String> },
    Dotenvy { key: Option<String> },
}

impl SecretRef {
    pub fn provider(&self) -> Provider {
        match self {
            Self::Env { .. }       => Provider::Env,
            Self::Op { .. }        => Provider::Op,
            Self::Dotenvx { .. }   => Provider::Dotenvx,
            Self::Sops { .. }      => Provider::Sops,
            Self::Gsm { .. }       => Provider::Gsm,
            Self::Nuenv { .. }     => Provider::Nuenv,
            Self::Direnv { .. }    => Provider::Direnv,
            Self::Mise { .. }      => Provider::Mise,
            Self::Bitwarden { .. } => Provider::Bitwarden,
            Self::Vault { .. }     => Provider::Vault,
            Self::Dotenvy { .. }   => Provider::Dotenvy,
        }
    }
}
```

#### Config root

```rust
#[derive(Debug, Deserialize)]
pub struct SecretsConfig {
    pub providers: Vec<Provider>,
    #[serde(default)]
    pub provider: HashMap<Provider, ProviderConfig>,
    #[serde(default)]
    pub secrets: HashMap<String, SecretRef>,
}
```

### Compile-Time Guarantees

- Misspelled provider name in TOML -> serde deserialization error
- Binding with wrong fields for a provider -> serde error
- `from_config` validates every binding references a provider in the chain
  (enum comparison, not string matching)
- `resolve_ref` receives typed variant; compiler warns on missing arms

### Domain Error

```rust
#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("[{name}] {source}")]
    SourceError { name: String, source: anyhow::Error },
    #[error("no provider resolved key: {key}")]
    NotFound { key: String },
    #[error("config error: {0}")]
    Config(String),
}
```

Adapters map infra errors (reqwest, tonic, serde, io) into
`SecretsError::SourceError`. Domain logic never sees infra error types.

### SecretResolver (domain logic)

```rust
pub struct SecretResolver {
    config: SecretsConfig,
    sources: Vec<Box<dyn SecretSource>>,
    enumerable: Vec<Box<dyn EnumerableSecretSource>>,
}

impl SecretResolver {
    pub fn from_config(config: SecretsConfig) -> Result<Self, SecretsError>;
    pub fn resolve(&self, key: &str) -> Result<Option<String>, SecretsError>;
    pub fn resolve_all(&self) -> Result<HashMap<String, String>, SecretsError>;
}
```

#### resolve(key) flow

1. Check explicit bindings (`config.secrets`). If key has a `SecretRef`,
   route to the named source via `source.resolve_ref(key, &secret_ref)`.
2. Otherwise, walk the priority chain (`sources` in order).
   First `Some` wins.

#### resolve_all() flow

1. Iterate enumerable sources in priority order. Merge maps -- earlier
   source wins on key collision.
2. Overlay explicit bindings: resolve each bound key individually via
   `resolve_ref`. Bindings always override chain results.

### No inject_env()

SRP: env injection (`set_var`) is a side effect belonging to the consumer
(notstrap), not the resolver. `SecretResolver` returns data; the caller
decides what to do with it.

## Config File

Separate `notsecrets.toml` at the dotfiles root. Rationale:

- notsecrets is consumed by notstrap, notfiles, and potentially standalone
- Provider chain config is complex enough to warrant its own file
- Follows existing pattern (each crate has its own state file)
- Enables standalone CLI usage: `notsecrets resolve DATABASE_URL`

### Example

```toml
providers = ["env", "op", "dotenvx", "sops", "gsm"]

[provider.op]
type = "op"
account = "my.1password.com"

[provider.dotenvx]
type = "dotenvx"
env_file = "~/dev/.env"

[provider.sops]
type = "sops"
file = "secrets/bootstrap.sops.env"

[provider.gsm]
type = "gsm"
project = "my-gcp-project"

[secrets]
DATABASE_URL = { source = "op", uri = "op://Personal/db/url" }
ANTHROPIC_API_KEY = { source = "dotenvx" }
GCP_TOKEN = { source = "gsm", path = "projects/123/secrets/gcp-token/versions/latest" }
```

## File Layout

New and modified files in `crates/notsecrets/src/`:

```
src/
  lib.rs              # add re-exports for new public API
  ports.rs            # add SecretSource, EnumerableSecretSource
  config.rs           # NEW -- SecretsConfig, ProviderConfig, SecretRef, Provider, load_config()
  resolver.rs         # NEW -- SecretResolver
  error.rs            # add SecretsError alongside existing AgeError
  sources/
    mod.rs            # add new source re-exports
    env.rs            # NEW tier 1 -- SecretSource + EnumerableSecretSource
    op.rs             # NEW tier 1 -- SecretSource only
    dotenvx.rs        # NEW tier 1 -- SecretSource + EnumerableSecretSource
    sops.rs           # NEW tier 1 -- SecretSource + EnumerableSecretSource
    gsm.rs            # NEW tier 1 -- SecretSource only
    nuenv.rs          # NEW tier 2 stub
    direnv.rs         # NEW tier 2 stub
    mise.rs           # NEW tier 2 stub
    vault.rs          # NEW tier 2 stub
    dotenvy.rs        # NEW tier 2 stub
    bitwarden.rs      # existing -- add tier 2 SecretSource stub
    file.rs           # existing -- untouched
    prompt.rs         # existing -- untouched
    yubikey.rs        # existing -- untouched
  identities/         # untouched
  recipients/         # untouched
  decrypt.rs          # untouched
  encrypt.rs          # untouched
  format.rs           # untouched
```

## Consumer Changes

### notstrap

Current hardcoded source chain (`lib.rs:135-143`) and `EnvInjector` closure
replaced by:

```rust
let secrets_config = notsecrets::config::load_config(
    &dotfiles_dir.join("notsecrets.toml")
)?;
let resolver = SecretResolver::from_config(secrets_config)?;
let env_map = resolver.resolve_all()?;

for (k, v) in &env_map {
    if let Some((k, v)) = parse_env_line(&format!("{k}={v}"))? {
        unsafe { std::env::set_var(&k, &v); }
    }
}
report.add("secrets", StepStatus::Ok);
```

`EnvInjector` type alias and `env_injector` field on `BootstrapOptions` become
unnecessary and are removed.

## Testing Strategy

| Dimension     | Scope                                               | When                          |
| ------------- | --------------------------------------------------- | ----------------------------- |
| Unit          | Resolver routing (binding vs chain), config parsing  | Every new function            |
| Conformance   | Shared suite for SecretSource trait, all impls       | Every new impl                |
| Property      | Config deserialization: arbitrary TOML never panics   | After config types defined    |
| Integration   | Resolver + real EnvSource + fake sources             | After resolver + 2+ sources   |
| Regression    | One test per bug                                     | Ongoing                       |
| Fuzz          | Deferred -- no raw byte parsing or unsafe            | Revisit if SOPS moves in-crate |
| Model check   | Deferred -- no arithmetic invariants                 | N/A                           |

### Conformance suite

```rust
fn assert_secret_source_contract(source: &dyn SecretSource) {
    // 1. name() returns non-empty string
    // 2. provider() matches expected variant
    // 3. resolve() for unknown key returns Ok(None), not Err
    // 4. resolve_ref() with mismatched variant returns Ok(None) or
    //    delegates to resolve()
}

fn assert_enumerable_contract(source: &dyn EnumerableSecretSource) {
    assert_secret_source_contract(source);
    // 5. resolve_all() returns Ok(map)
    // 6. For every key in resolve_all() map,
    //    resolve(key) returns Ok(Some(same_value))
}
```

## Implementation Tiers

### Tier 1 (first milestone)

- Types: `Provider`, `ProviderConfig`, `SecretRef`, `SecretsConfig`, `SecretsError`
- Traits: `SecretSource`, `EnumerableSecretSource`
- Resolver: `SecretResolver` with `from_config`, `resolve`, `resolve_all`
- Sources: `EnvSource`, `OpSource`, `DotenvxSource`, `SopsSource`, `GsmSource`
- Config: `load_config()`, `notsecrets.toml` parsing
- Consumer: notstrap integration
- Tests: unit + conformance + integration

### Tier 2 (subsequent)

- Sources: `NuenvSource`, `DirenvSource`, `MiseSource`, `BitwardenSource`
  (SecretSource impl), `VaultSource`, `DotenvySource`
- Each: trait impl returning `Ok(None)` / empty map, passing conformance suite
- Implemented as needed when the backend is required

## Decisions Log

| Decision                          | Why                                                    | Alternative rejected               |
| --------------------------------- | ------------------------------------------------------ | ---------------------------------- |
| Separate `notsecrets.toml`        | Decouples from notfiles/notstrap config                | Subsection in notfiles.toml        |
| Two traits (ISP split)            | Not all backends can enumerate                         | Single trait with `resolve_all()`  |
| Typed enums over string maps      | Compile-time validation, exhaustive matching           | `HashMap<String, toml::Value>`     |
| No `inject_env()` on resolver     | SRP -- side effects belong to consumer                 | Method on SecretResolver           |
| `resolve_ref` default impl        | Sources that don't use refs delegate to `resolve(key)` | Per-binding source instances       |
| Explicit bindings override chain  | User intent is unambiguous when they bind a key        | Chain-only, no bindings            |
| `SecretsError` separate from `AgeError` | Different domains, different consumers            | Unified error enum                 |

## Out of Scope

- Secret write/rotation path
- RBAC or access control on secrets
- Caching/TTL for resolved secrets
- Async trait (all providers are sync CLI/file-based today)
- Migration tooling from old notstrap config format
