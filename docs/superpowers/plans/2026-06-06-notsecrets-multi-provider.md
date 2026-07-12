# Plan: notsecrets Multi-Provider Secret Resolution

## Goal

Add a config-driven, strongly-typed secret resolution system to notsecrets with
11 provider backends, explicit key bindings, and a priority chain fallback.

## Context Map

See `docs/superpowers/specs/2026-06-06-notsecrets-multi-provider-design.md` for
full design. Context map produced at plan time:

- **Modified**: `ports.rs`, `error.rs`, `lib.rs`, `sources/mod.rs`,
  `sources/bitwarden.rs`, `Cargo.toml`, `notstrap/src/lib.rs`
- **Created**: `config.rs`, `resolver.rs`, 10 new source files
- **Consumers**: notstrap (primary), cross-crate integration tests
- **Risk**: all changes additive except notstrap `EnvInjector` removal (binary
  crate, safe)

## Architecture

- **Crate**: `notsecrets` (library)
- **New types**: `Provider`, `ProviderConfig`, `SecretRef`, `SecretsConfig`,
  `SecretsError`, `SecretSource`, `EnumerableSecretSource`, `SecretResolver`
- **New files**: `config.rs`, `resolver.rs`, 10 source adapter files
- **Data flow**: `notsecrets.toml` -> `SecretsConfig` -> `SecretResolver` ->
  `SecretSource` impls -> resolved `HashMap<String, String>`

## Tech Stack

- Rust edition 2024
- New deps on notsecrets: `serde` (with `derive`), `toml`
- Tier 1 sources shell out to CLIs: `op` (1Password), `dotenvx`, `sops`,
  `gcloud` (GSM). `EnvSource` uses `std::env`.

## Tasks

### Task 1: Add serde/toml deps and SecretsError

**Crate**: `notsecrets`
**File(s)**: `crates/notsecrets/Cargo.toml`, `crates/notsecrets/src/error.rs`
**Run**: `cargo check -p notsecrets`

1. Write failing test:

   ```rust
   // crates/notsecrets/src/error.rs
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn secrets_error_display_source_error() {
           let err = SecretsError::SourceError {
               name: "op".to_string(),
               source: anyhow::anyhow!("not found"),
           };
           assert!(err.to_string().contains("[op]"));
           assert!(err.to_string().contains("not found"));
       }

       #[test]
       fn secrets_error_display_not_found() {
           let err = SecretsError::NotFound {
               key: "DB_URL".to_string(),
           };
           assert!(err.to_string().contains("DB_URL"));
       }

       #[test]
       fn secrets_error_display_config() {
           let err = SecretsError::Config("bad toml".to_string());
           assert!(err.to_string().contains("bad toml"));
       }
   }
   ```

   Run: `cargo nextest run -p notsecrets -- secrets_error`
   Expected: FAIL (SecretsError does not exist)

2. Add deps to `Cargo.toml`:

   ```toml
   serde = { workspace = true, features = ["derive"] }
   toml = { workspace = true }
   ```

   If `serde` and `toml` are not in workspace deps, add them to root
   `Cargo.toml` `[workspace.dependencies]` first.

3. Add `SecretsError` to `error.rs`:

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

4. Add `pub use error::SecretsError;` to `lib.rs`.

5. Verify:

   ```
   cargo nextest run -p notsecrets -- secrets_error  -> all green
   cargo clippy -p notsecrets -- -D warnings         -> zero warnings
   ```

6. Run: `git branch --show-current` (verify branch)
   Commit: `feat(notsecrets): add SecretsError and serde/toml deps`

---

### Task 2: Provider enum and config types

**Crate**: `notsecrets`
**File(s)**: `crates/notsecrets/src/config.rs`, `crates/notsecrets/src/lib.rs`
**Run**: `cargo nextest run -p notsecrets -- config`

1. Write failing test:

   ```rust
   // crates/notsecrets/src/config.rs
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn provider_roundtrip_serde() {
           let p = Provider::Op;
           let s = toml::to_string(&p).unwrap();
           let p2: Provider = toml::from_str(&s).unwrap();
           assert_eq!(p, p2);
       }

       #[test]
       fn secrets_config_parses_minimal() {
           let toml_str = r#"
               providers = ["env", "op"]
           "#;
           let cfg: SecretsConfig = toml::from_str(toml_str).unwrap();
           assert_eq!(cfg.providers, vec![Provider::Env, Provider::Op]);
           assert!(cfg.provider.is_empty());
           assert!(cfg.secrets.is_empty());
       }

       #[test]
       fn secrets_config_parses_full() {
           let toml_str = r#"
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
           "#;
           let cfg: SecretsConfig = toml::from_str(toml_str).unwrap();
           assert_eq!(cfg.providers.len(), 5);
           assert_eq!(cfg.provider.len(), 4);
           assert_eq!(cfg.secrets.len(), 3);
           assert_eq!(
               cfg.secrets["DATABASE_URL"].provider(),
               Provider::Op
           );
       }

       #[test]
       fn secret_ref_provider_derivation() {
           let r = SecretRef::Gsm {
               path: "projects/123/secrets/tok/versions/latest".to_string(),
           };
           assert_eq!(r.provider(), Provider::Gsm);
       }

       #[test]
       fn unknown_provider_in_toml_errors() {
           let toml_str = r#"providers = ["env", "redis"]"#;
           let result: Result<SecretsConfig, _> = toml::from_str(toml_str);
           assert!(result.is_err());
       }

       #[test]
       fn load_config_reads_file() {
           let dir = tempfile::TempDir::new().unwrap();
           let path = dir.path().join("notsecrets.toml");
           std::fs::write(&path, "providers = [\"env\"]\n").unwrap();
           let cfg = load_config(&path).unwrap();
           assert_eq!(cfg.providers, vec![Provider::Env]);
       }

       #[test]
       fn load_config_missing_file_errors() {
           let result = load_config(std::path::Path::new("/nonexistent/notsecrets.toml"));
           assert!(result.is_err());
       }
   }
   ```

   Run: `cargo nextest run -p notsecrets -- config`
   Expected: FAIL (config module does not exist)

2. Create `crates/notsecrets/src/config.rs`:

   ```rust
   use crate::error::SecretsError;
   use serde::{Deserialize, Serialize};
   use std::collections::HashMap;
   use std::path::{Path, PathBuf};

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
               Self::Env { .. } => Provider::Env,
               Self::Op { .. } => Provider::Op,
               Self::Dotenvx { .. } => Provider::Dotenvx,
               Self::Sops { .. } => Provider::Sops,
               Self::Gsm { .. } => Provider::Gsm,
               Self::Nuenv { .. } => Provider::Nuenv,
               Self::Direnv { .. } => Provider::Direnv,
               Self::Mise { .. } => Provider::Mise,
               Self::Bitwarden { .. } => Provider::Bitwarden,
               Self::Vault { .. } => Provider::Vault,
               Self::Dotenvy { .. } => Provider::Dotenvy,
           }
       }
   }

   #[derive(Debug, Deserialize)]
   pub struct SecretsConfig {
       pub providers: Vec<Provider>,
       #[serde(default)]
       pub provider: HashMap<Provider, ProviderConfig>,
       #[serde(default)]
       pub secrets: HashMap<String, SecretRef>,
   }

   pub fn load_config(path: &Path) -> Result<SecretsConfig, SecretsError> {
       let content = std::fs::read_to_string(path).map_err(|e| {
           SecretsError::Config(format!("cannot read {}: {e}", path.display()))
       })?;
       toml::from_str(&content)
           .map_err(|e| SecretsError::Config(format!("parse error: {e}")))
   }
   ```

3. Add `pub mod config;` to `lib.rs`. Add re-exports:

   ```rust
   pub use config::{Provider, ProviderConfig, SecretRef, SecretsConfig, load_config};
   ```

4. Verify:

   ```
   cargo nextest run -p notsecrets -- config  -> all green
   cargo clippy -p notsecrets -- -D warnings  -> zero warnings
   ```

5. Commit: `feat(notsecrets): add Provider enum and typed config types`

---

### Task 3: SecretSource and EnumerableSecretSource traits

**Crate**: `notsecrets`
**File(s)**: `crates/notsecrets/src/ports.rs`, `crates/notsecrets/src/lib.rs`
**Run**: `cargo check -p notsecrets`

1. Write failing test:

   ```rust
   // crates/notsecrets/src/ports.rs
   #[cfg(test)]
   mod tests {
       use super::*;
       use crate::config::{Provider, SecretRef};
       use std::collections::HashMap;

       struct FakeSource;
       impl SecretSource for FakeSource {
           fn name(&self) -> &str { "fake" }
           fn provider(&self) -> Provider { Provider::Env }
           fn resolve(&self, _key: &str) -> Result<Option<String>, SecretsError> {
               Ok(Some("val".to_string()))
           }
       }

       #[test]
       fn default_resolve_ref_delegates_to_resolve() {
           let source = FakeSource;
           let secret_ref = SecretRef::Env;
           let result = source.resolve_ref("KEY", &secret_ref).unwrap();
           assert_eq!(result, Some("val".to_string()));
       }
   }
   ```

   Run: `cargo nextest run -p notsecrets -- default_resolve_ref`
   Expected: FAIL (SecretSource does not exist)

2. Add to `ports.rs`:

   ```rust
   use crate::config::{Provider, SecretRef};
   use crate::error::SecretsError;
   use std::collections::HashMap;

   pub trait SecretSource {
       fn name(&self) -> &str;
       fn provider(&self) -> Provider;
       fn resolve(&self, key: &str) -> Result<Option<String>, SecretsError>;
       fn resolve_ref(
           &self,
           key: &str,
           _secret_ref: &SecretRef,
       ) -> Result<Option<String>, SecretsError> {
           self.resolve(key)
       }
   }

   pub trait EnumerableSecretSource: SecretSource {
       fn resolve_all(&self) -> Result<HashMap<String, String>, SecretsError>;
   }
   ```

3. Add re-exports to `lib.rs`:

   ```rust
   pub use ports::{SecretSource, EnumerableSecretSource};
   ```

4. Verify:

   ```
   cargo nextest run -p notsecrets -- default_resolve_ref  -> green
   cargo clippy -p notsecrets -- -D warnings               -> zero warnings
   ```

5. Commit: `feat(notsecrets): add SecretSource and EnumerableSecretSource traits`

---

### Task 4: EnvSource (tier 1)

**Crate**: `notsecrets`
**File(s)**: `crates/notsecrets/src/sources/env.rs`, `crates/notsecrets/src/sources/mod.rs`
**Run**: `cargo nextest run -p notsecrets -- env_source`

1. Write failing test:

   ```rust
   // crates/notsecrets/src/sources/env.rs
   #[cfg(test)]
   mod tests {
       use super::*;
       use crate::ports::{SecretSource, EnumerableSecretSource};

       #[test]
       fn env_source_resolve_existing_key() {
           // SAFETY: test is single-threaded
           unsafe { std::env::set_var("NOTSECRETS_TEST_KEY", "test_value"); }
           let source = EnvSource;
           let result = source.resolve("NOTSECRETS_TEST_KEY").unwrap();
           assert_eq!(result, Some("test_value".to_string()));
           unsafe { std::env::remove_var("NOTSECRETS_TEST_KEY"); }
       }

       #[test]
       fn env_source_resolve_missing_key() {
           let source = EnvSource;
           let result = source.resolve("NOTSECRETS_NONEXISTENT_KEY_12345").unwrap();
           assert_eq!(result, None);
       }

       #[test]
       fn env_source_resolve_all_contains_path() {
           let source = EnvSource;
           let map = source.resolve_all().unwrap();
           // PATH is virtually always set
           assert!(map.contains_key("HOME") || map.contains_key("USER"));
       }

       #[test]
       fn env_source_name_and_provider() {
           let source = EnvSource;
           assert_eq!(source.name(), "env");
           assert_eq!(source.provider(), Provider::Env);
       }
   }
   ```

   Expected: FAIL

2. Create `crates/notsecrets/src/sources/env.rs`:

   ```rust
   use crate::config::{Provider, SecretRef};
   use crate::error::SecretsError;
   use crate::ports::{EnumerableSecretSource, SecretSource};
   use std::collections::HashMap;

   pub struct EnvSource;

   impl SecretSource for EnvSource {
       fn name(&self) -> &str {
           "env"
       }

       fn provider(&self) -> Provider {
           Provider::Env
       }

       fn resolve(&self, key: &str) -> Result<Option<String>, SecretsError> {
           Ok(std::env::var(key).ok())
       }
   }

   impl EnumerableSecretSource for EnvSource {
       fn resolve_all(&self) -> Result<HashMap<String, String>, SecretsError> {
           Ok(std::env::vars().collect())
       }
   }
   ```

3. Add `pub mod env;` and `pub use env::EnvSource;` to `sources/mod.rs`.

4. Verify:

   ```
   cargo nextest run -p notsecrets -- env_source  -> all green
   cargo clippy -p notsecrets -- -D warnings      -> zero warnings
   ```

5. Commit: `feat(notsecrets): add EnvSource (tier 1 provider)`

---

### Task 5: OpSource (tier 1)

**Crate**: `notsecrets`
**File(s)**: `crates/notsecrets/src/sources/op.rs`, `crates/notsecrets/src/sources/mod.rs`
**Run**: `cargo nextest run -p notsecrets -- op_source`

1. Write failing test:

   ```rust
   // crates/notsecrets/src/sources/op.rs
   #[cfg(test)]
   mod tests {
       use super::*;
       use crate::config::SecretRef;
       use crate::ports::SecretSource;

       #[test]
       fn op_source_name_and_provider() {
           let source = OpSource::new("my.1password.com".to_string());
           assert_eq!(source.name(), "op");
           assert_eq!(source.provider(), Provider::Op);
       }

       #[test]
       fn op_source_resolve_without_ref_returns_none() {
           // Without a ref, OpSource cannot look up by key name alone
           let source = OpSource::new("my.1password.com".to_string());
           let result = source.resolve("SOME_KEY").unwrap();
           assert_eq!(result, None);
       }

       #[test]
       fn op_source_resolve_ref_wrong_variant_delegates() {
           let source = OpSource::new("my.1password.com".to_string());
           let wrong_ref = SecretRef::Env;
           let result = source.resolve_ref("KEY", &wrong_ref).unwrap();
           assert_eq!(result, None);
       }
   }
   ```

   Expected: FAIL

2. Create `crates/notsecrets/src/sources/op.rs`:

   ```rust
   use crate::config::{Provider, SecretRef};
   use crate::error::SecretsError;
   use crate::ports::SecretSource;
   use std::process::Command;

   pub struct OpSource {
       account: String,
   }

   impl OpSource {
       pub fn new(account: String) -> Self {
           Self { account }
       }
   }

   impl SecretSource for OpSource {
       fn name(&self) -> &str {
           "op"
       }

       fn provider(&self) -> Provider {
           Provider::Op
       }

       /// OpSource cannot resolve by key name alone -- requires a ref.
       fn resolve(&self, _key: &str) -> Result<Option<String>, SecretsError> {
           Ok(None)
       }

       fn resolve_ref(
           &self,
           _key: &str,
           secret_ref: &SecretRef,
       ) -> Result<Option<String>, SecretsError> {
           let SecretRef::Op { uri } = secret_ref else {
               return self.resolve(_key);
           };
           let output = Command::new("op")
               .args(["read", uri, "--account", &self.account])
               .output()
               .map_err(|e| SecretsError::SourceError {
                   name: "op".to_string(),
                   source: e.into(),
               })?;
           if output.status.success() {
               Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_string()))
           } else {
               let stderr = String::from_utf8_lossy(&output.stderr);
               Err(SecretsError::SourceError {
                   name: "op".to_string(),
                   source: anyhow::anyhow!("op read failed: {stderr}"),
               })
           }
       }
   }
   ```

3. Add to `sources/mod.rs`: `pub mod op;` and `pub use op::OpSource;`

4. Verify:

   ```
   cargo nextest run -p notsecrets -- op_source  -> all green
   cargo clippy -p notsecrets -- -D warnings     -> zero warnings
   ```

5. Commit: `feat(notsecrets): add OpSource (tier 1 provider)`

---

### Task 6: DotenvxSource (tier 1)

**Crate**: `notsecrets`
**File(s)**: `crates/notsecrets/src/sources/dotenvx.rs`, `crates/notsecrets/src/sources/mod.rs`
**Run**: `cargo nextest run -p notsecrets -- dotenvx_source`

1. Write failing test:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use crate::ports::{SecretSource, EnumerableSecretSource};

       #[test]
       fn dotenvx_source_name_and_provider() {
           let source = DotenvxSource::new("/tmp/test.env".into());
           assert_eq!(source.name(), "dotenvx");
           assert_eq!(source.provider(), Provider::Dotenvx);
       }

       #[test]
       fn dotenvx_source_resolve_missing_file_returns_error() {
           let source = DotenvxSource::new("/nonexistent/.env".into());
           let result = source.resolve_all();
           assert!(result.is_err());
       }
   }
   ```

   Expected: FAIL

2. Create `crates/notsecrets/src/sources/dotenvx.rs`:

   ```rust
   use crate::config::{Provider, SecretRef};
   use crate::error::SecretsError;
   use crate::ports::{EnumerableSecretSource, SecretSource};
   use std::collections::HashMap;
   use std::path::PathBuf;
   use std::process::Command;

   pub struct DotenvxSource {
       env_file: PathBuf,
   }

   impl DotenvxSource {
       pub fn new(env_file: PathBuf) -> Self {
           Self { env_file }
       }

       fn decrypt_all(&self) -> Result<HashMap<String, String>, SecretsError> {
           let output = Command::new("dotenvx")
               .args(["run", "--env-file", &self.env_file.to_string_lossy(), "--", "env"])
               .output()
               .map_err(|e| SecretsError::SourceError {
                   name: "dotenvx".to_string(),
                   source: e.into(),
               })?;
           if !output.status.success() {
               let stderr = String::from_utf8_lossy(&output.stderr);
               return Err(SecretsError::SourceError {
                   name: "dotenvx".to_string(),
                   source: anyhow::anyhow!("dotenvx run failed: {stderr}"),
               });
           }
           let stdout = String::from_utf8_lossy(&output.stdout);
           let mut map = HashMap::new();
           for line in stdout.lines() {
               if let Some((k, v)) = line.split_once('=') {
                   let k = k.trim();
                   if !k.is_empty() && !k.starts_with('#') {
                       map.insert(k.to_string(), v.to_string());
                   }
               }
           }
           Ok(map)
       }
   }

   impl SecretSource for DotenvxSource {
       fn name(&self) -> &str { "dotenvx" }
       fn provider(&self) -> Provider { Provider::Dotenvx }

       fn resolve(&self, key: &str) -> Result<Option<String>, SecretsError> {
           self.decrypt_all().map(|m| m.get(key).cloned())
       }
   }

   impl EnumerableSecretSource for DotenvxSource {
       fn resolve_all(&self) -> Result<HashMap<String, String>, SecretsError> {
           self.decrypt_all()
       }
   }
   ```

3. Add to `sources/mod.rs`.

4. Verify, commit: `feat(notsecrets): add DotenvxSource (tier 1 provider)`

---

### Task 7: SopsSource (tier 1)

**Crate**: `notsecrets`
**File(s)**: `crates/notsecrets/src/sources/sops.rs`, `crates/notsecrets/src/sources/mod.rs`
**Run**: `cargo nextest run -p notsecrets -- sops_source`

1. Write failing test:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use crate::ports::SecretSource;

       #[test]
       fn sops_source_name_and_provider() {
           let source = SopsSource::new("/tmp/secrets.sops.env".into());
           assert_eq!(source.name(), "sops");
           assert_eq!(source.provider(), Provider::Sops);
       }

       #[test]
       fn sops_source_resolve_missing_file_returns_error() {
           let source = SopsSource::new("/nonexistent/secrets.sops.env".into());
           let result = source.resolve("KEY");
           assert!(result.is_err());
       }
   }
   ```

   Expected: FAIL

2. Create `crates/notsecrets/src/sources/sops.rs`:

   ```rust
   use crate::config::{Provider, SecretRef};
   use crate::error::SecretsError;
   use crate::ports::{EnumerableSecretSource, SecretSource};
   use std::collections::HashMap;
   use std::path::PathBuf;
   use std::process::Command;

   pub struct SopsSource {
       file: PathBuf,
   }

   impl SopsSource {
       pub fn new(file: PathBuf) -> Self {
           Self { file }
       }

       fn decrypt_all(&self) -> Result<HashMap<String, String>, SecretsError> {
           let output = Command::new("sops")
               .args(["--decrypt", &self.file.to_string_lossy()])
               .output()
               .map_err(|e| SecretsError::SourceError {
                   name: "sops".to_string(),
                   source: e.into(),
               })?;
           if !output.status.success() {
               let stderr = String::from_utf8_lossy(&output.stderr);
               return Err(SecretsError::SourceError {
                   name: "sops".to_string(),
                   source: anyhow::anyhow!("sops decrypt failed: {stderr}"),
               });
           }
           let stdout = String::from_utf8_lossy(&output.stdout);
           let mut map = HashMap::new();
           for line in stdout.lines() {
               if let Some((k, v)) = line.split_once('=') {
                   let k = k.trim();
                   if !k.is_empty() && !k.starts_with('#') {
                       map.insert(k.to_string(), v.to_string());
                   }
               }
           }
           Ok(map)
       }
   }

   impl SecretSource for SopsSource {
       fn name(&self) -> &str { "sops" }
       fn provider(&self) -> Provider { Provider::Sops }

       fn resolve(&self, key: &str) -> Result<Option<String>, SecretsError> {
           self.decrypt_all().map(|m| m.get(key).cloned())
       }
   }

   impl EnumerableSecretSource for SopsSource {
       fn resolve_all(&self) -> Result<HashMap<String, String>, SecretsError> {
           self.decrypt_all()
       }
   }
   ```

3. Add to `sources/mod.rs`.

4. Verify, commit: `feat(notsecrets): add SopsSource (tier 1 provider)`

---

### Task 8: GsmSource (tier 1)

**Crate**: `notsecrets`
**File(s)**: `crates/notsecrets/src/sources/gsm.rs`, `crates/notsecrets/src/sources/mod.rs`
**Run**: `cargo nextest run -p notsecrets -- gsm_source`

1. Write failing test:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use crate::config::SecretRef;
       use crate::ports::SecretSource;

       #[test]
       fn gsm_source_name_and_provider() {
           let source = GsmSource::new("my-project".to_string());
           assert_eq!(source.name(), "gsm");
           assert_eq!(source.provider(), Provider::Gsm);
       }

       #[test]
       fn gsm_source_resolve_without_ref_returns_none() {
           let source = GsmSource::new("my-project".to_string());
           let result = source.resolve("KEY").unwrap();
           assert_eq!(result, None);
       }

       #[test]
       fn gsm_source_resolve_ref_wrong_variant_delegates() {
           let source = GsmSource::new("my-project".to_string());
           let wrong_ref = SecretRef::Env;
           let result = source.resolve_ref("KEY", &wrong_ref).unwrap();
           assert_eq!(result, None);
       }
   }
   ```

   Expected: FAIL

2. Create `crates/notsecrets/src/sources/gsm.rs`:

   ```rust
   use crate::config::{Provider, SecretRef};
   use crate::error::SecretsError;
   use crate::ports::SecretSource;
   use std::process::Command;

   pub struct GsmSource {
       project: String,
   }

   impl GsmSource {
       pub fn new(project: String) -> Self {
           Self { project }
       }
   }

   impl SecretSource for GsmSource {
       fn name(&self) -> &str { "gsm" }
       fn provider(&self) -> Provider { Provider::Gsm }

       fn resolve(&self, _key: &str) -> Result<Option<String>, SecretsError> {
           Ok(None)
       }

       fn resolve_ref(
           &self,
           _key: &str,
           secret_ref: &SecretRef,
       ) -> Result<Option<String>, SecretsError> {
           let SecretRef::Gsm { path } = secret_ref else {
               return self.resolve(_key);
           };
           let output = Command::new("gcloud")
               .args([
                   "secrets", "versions", "access", "latest",
                   "--secret", path,
                   "--project", &self.project,
               ])
               .output()
               .map_err(|e| SecretsError::SourceError {
                   name: "gsm".to_string(),
                   source: e.into(),
               })?;
           if output.status.success() {
               Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_string()))
           } else {
               let stderr = String::from_utf8_lossy(&output.stderr);
               Err(SecretsError::SourceError {
                   name: "gsm".to_string(),
                   source: anyhow::anyhow!("gcloud secrets failed: {stderr}"),
               })
           }
       }
   }
   ```

3. Add to `sources/mod.rs`.

4. Verify, commit: `feat(notsecrets): add GsmSource (tier 1 provider)`

---

### Task 9: Tier 2 stubs (6 sources)

**Crate**: `notsecrets`
**File(s)**: `crates/notsecrets/src/sources/{nuenv,direnv,mise,vault,dotenvy}.rs`,
  `crates/notsecrets/src/sources/bitwarden.rs` (add SecretSource impl),
  `crates/notsecrets/src/sources/mod.rs`
**Run**: `cargo nextest run -p notsecrets -- stub`

1. Write failing test (one representative, same pattern for all):

   ```rust
   // crates/notsecrets/src/sources/nuenv.rs
   #[cfg(test)]
   mod tests {
       use super::*;
       use crate::ports::SecretSource;

       #[test]
       fn nuenv_stub_resolve_returns_none() {
           let source = NuenvSource;
           assert_eq!(source.resolve("ANY").unwrap(), None);
       }

       #[test]
       fn nuenv_stub_name_and_provider() {
           let source = NuenvSource;
           assert_eq!(source.name(), "nuenv");
           assert_eq!(source.provider(), Provider::Nuenv);
       }
   }
   ```

   Expected: FAIL

2. Create each stub file. Template (using nuenv as example):

   ```rust
   use crate::config::Provider;
   use crate::error::SecretsError;
   use crate::ports::SecretSource;

   pub struct NuenvSource;

   impl SecretSource for NuenvSource {
       fn name(&self) -> &str { "nuenv" }
       fn provider(&self) -> Provider { Provider::Nuenv }
       fn resolve(&self, _key: &str) -> Result<Option<String>, SecretsError> {
           Ok(None)
       }
   }
   ```

   Repeat for: `DirenvSource` (Provider::Direnv), `MiseSource` (Provider::Mise),
   `VaultSource` (Provider::Vault), `DotenvySource` (Provider::Dotenvy).

3. Add `SecretSource` stub impl to existing `bitwarden.rs`:

   ```rust
   impl SecretSource for BitwardenSource {
       fn name(&self) -> &str { "bitwarden" }
       fn provider(&self) -> Provider { Provider::Bitwarden }
       fn resolve(&self, _key: &str) -> Result<Option<String>, SecretsError> {
           Ok(None)
       }
   }
   ```

4. Update `sources/mod.rs` with all new modules and re-exports.

5. Verify:

   ```
   cargo nextest run -p notsecrets -- stub  -> all green
   cargo clippy -p notsecrets -- -D warnings -> zero warnings
   ```

6. Commit: `feat(notsecrets): add tier 2 SecretSource stubs`

---

### Task 10: SecretResolver

**Crate**: `notsecrets`
**File(s)**: `crates/notsecrets/src/resolver.rs`, `crates/notsecrets/src/lib.rs`
**Run**: `cargo nextest run -p notsecrets -- resolver`

1. Write failing tests:

   ```rust
   // crates/notsecrets/src/resolver.rs
   #[cfg(test)]
   mod tests {
       use super::*;
       use crate::config::{Provider, SecretRef, SecretsConfig};
       use std::collections::HashMap;

       fn config_with_env_only() -> SecretsConfig {
           SecretsConfig {
               providers: vec![Provider::Env],
               provider: HashMap::new(),
               secrets: HashMap::new(),
           }
       }

       #[test]
       fn resolver_from_config_env_only() {
           let resolver = SecretResolver::from_config(config_with_env_only());
           assert!(resolver.is_ok());
       }

       #[test]
       fn resolver_resolve_env_var() {
           unsafe { std::env::set_var("NOTSECRETS_RESOLVER_TEST", "hello"); }
           let resolver = SecretResolver::from_config(config_with_env_only()).unwrap();
           let val = resolver.resolve("NOTSECRETS_RESOLVER_TEST").unwrap();
           assert_eq!(val, Some("hello".to_string()));
           unsafe { std::env::remove_var("NOTSECRETS_RESOLVER_TEST"); }
       }

       #[test]
       fn resolver_resolve_missing_returns_none() {
           let resolver = SecretResolver::from_config(config_with_env_only()).unwrap();
           let val = resolver.resolve("NOTSECRETS_NONEXISTENT_99999").unwrap();
           assert_eq!(val, None);
       }

       #[test]
       fn resolver_binding_overrides_chain() {
           unsafe { std::env::set_var("NOTSECRETS_BOUND_TEST", "from_env"); }
           let mut config = config_with_env_only();
           // Binding to env with no special ref -- should still resolve via env
           config.secrets.insert(
               "NOTSECRETS_BOUND_TEST".to_string(),
               SecretRef::Env,
           );
           let resolver = SecretResolver::from_config(config).unwrap();
           let val = resolver.resolve("NOTSECRETS_BOUND_TEST").unwrap();
           assert_eq!(val, Some("from_env".to_string()));
           unsafe { std::env::remove_var("NOTSECRETS_BOUND_TEST"); }
       }

       #[test]
       fn resolver_resolve_all_includes_env() {
           unsafe { std::env::set_var("NOTSECRETS_ALL_TEST", "present"); }
           let resolver = SecretResolver::from_config(config_with_env_only()).unwrap();
           let map = resolver.resolve_all().unwrap();
           assert_eq!(map.get("NOTSECRETS_ALL_TEST").map(|s| s.as_str()), Some("present"));
           unsafe { std::env::remove_var("NOTSECRETS_ALL_TEST"); }
       }

       #[test]
       fn resolver_binding_refs_unknown_provider_errors() {
           let config = SecretsConfig {
               providers: vec![Provider::Env],
               provider: HashMap::new(),
               secrets: HashMap::from([(
                   "KEY".to_string(),
                   SecretRef::Op { uri: "op://x/y/z".to_string() },
               )]),
           };
           let result = SecretResolver::from_config(config);
           assert!(result.is_err());
       }
   }
   ```

   Expected: FAIL

2. Create `crates/notsecrets/src/resolver.rs`:

   ```rust
   use crate::config::{Provider, ProviderConfig, SecretRef, SecretsConfig};
   use crate::error::SecretsError;
   use crate::ports::{EnumerableSecretSource, SecretSource};
   use crate::sources::*;
   use std::collections::HashMap;

   pub struct SecretResolver {
       config: SecretsConfig,
       sources: Vec<Box<dyn SecretSource>>,
       enumerable: Vec<Box<dyn EnumerableSecretSource>>,
   }

   impl SecretResolver {
       pub fn from_config(config: SecretsConfig) -> Result<Self, SecretsError> {
           // Validate: every binding references a provider in the chain
           for (key, binding) in &config.secrets {
               if !config.providers.contains(&binding.provider()) {
                   return Err(SecretsError::Config(format!(
                       "secret '{key}' references provider '{:?}' not in providers list",
                       binding.provider()
                   )));
               }
           }

           let mut sources: Vec<Box<dyn SecretSource>> = Vec::new();
           let mut enumerable: Vec<Box<dyn EnumerableSecretSource>> = Vec::new();

           for provider in &config.providers {
               let provider_config = config.provider.get(provider);
               match provider {
                   Provider::Env => {
                       let s = EnvSource;
                       enumerable.push(Box::new(EnvSource));
                       sources.push(Box::new(s));
                   }
                   Provider::Op => {
                       let account = match provider_config {
                           Some(ProviderConfig::Op { account }) => account.clone(),
                           _ => return Err(SecretsError::Config(
                               "op provider requires [provider.op] with account".to_string(),
                           )),
                       };
                       sources.push(Box::new(OpSource::new(account)));
                   }
                   Provider::Dotenvx => {
                       let env_file = match provider_config {
                           Some(ProviderConfig::Dotenvx { env_file }) => env_file.clone(),
                           _ => return Err(SecretsError::Config(
                               "dotenvx provider requires [provider.dotenvx] with env_file".to_string(),
                           )),
                       };
                       let s = DotenvxSource::new(env_file.clone());
                       enumerable.push(Box::new(DotenvxSource::new(env_file)));
                       sources.push(Box::new(s));
                   }
                   Provider::Sops => {
                       let file = match provider_config {
                           Some(ProviderConfig::Sops { file }) => file.clone(),
                           _ => return Err(SecretsError::Config(
                               "sops provider requires [provider.sops] with file".to_string(),
                           )),
                       };
                       let s = SopsSource::new(file.clone());
                       enumerable.push(Box::new(SopsSource::new(file)));
                       sources.push(Box::new(s));
                   }
                   Provider::Gsm => {
                       let project = match provider_config {
                           Some(ProviderConfig::Gsm { project }) => project.clone(),
                           _ => return Err(SecretsError::Config(
                               "gsm provider requires [provider.gsm] with project".to_string(),
                           )),
                       };
                       sources.push(Box::new(GsmSource::new(project)));
                   }
                   // Tier 2: no config required
                   Provider::Nuenv => {
                       sources.push(Box::new(NuenvSource));
                   }
                   Provider::Direnv => {
                       sources.push(Box::new(DirenvSource));
                   }
                   Provider::Mise => {
                       sources.push(Box::new(MiseSource));
                   }
                   Provider::Bitwarden => {
                       let item_name = match provider_config {
                           Some(ProviderConfig::Bitwarden { server_url: _ }) => {
                               "default".to_string()
                           }
                           _ => "default".to_string(),
                       };
                       sources.push(Box::new(BitwardenSource::new(item_name)));
                   }
                   Provider::Vault => {
                       sources.push(Box::new(VaultSource));
                   }
                   Provider::Dotenvy => {
                       sources.push(Box::new(DotenvySource));
                   }
               }
           }

           Ok(Self { config, sources, enumerable })
       }

       pub fn resolve(&self, key: &str) -> Result<Option<String>, SecretsError> {
           // 1. Explicit binding
           if let Some(secret_ref) = self.config.secrets.get(key) {
               let provider = secret_ref.provider();
               if let Some(source) = self.sources.iter().find(|s| s.provider() == provider) {
                   return source.resolve_ref(key, secret_ref);
               }
           }

           // 2. Priority chain
           for source in &self.sources {
               if let Some(value) = source.resolve(key)? {
                   return Ok(Some(value));
               }
           }
           Ok(None)
       }

       pub fn resolve_all(&self) -> Result<HashMap<String, String>, SecretsError> {
           let mut map = HashMap::new();

           // 1. Merge enumerable sources (priority order, first wins)
           for source in &self.enumerable {
               let source_map = source.resolve_all()?;
               for (k, v) in source_map {
                   map.entry(k).or_insert(v);
               }
           }

           // 2. Overlay explicit bindings (always win)
           for (key, secret_ref) in &self.config.secrets {
               let provider = secret_ref.provider();
               if let Some(source) = self.sources.iter().find(|s| s.provider() == provider) {
                   if let Some(value) = source.resolve_ref(key, secret_ref)? {
                       map.insert(key.clone(), value);
                   }
               }
           }

           Ok(map)
       }

       pub fn source_by_provider(&self, provider: Provider) -> Option<&dyn SecretSource> {
           self.sources.iter().find(|s| s.provider() == provider).map(|s| s.as_ref())
       }
   }
   ```

3. Add `pub mod resolver;` to `lib.rs`. Re-export:

   ```rust
   pub use resolver::SecretResolver;
   ```

4. Verify:

   ```
   cargo nextest run -p notsecrets -- resolver  -> all green
   cargo clippy -p notsecrets -- -D warnings    -> zero warnings
   ```

5. Commit: `feat(notsecrets): add SecretResolver with binding + chain resolution`

---

### Task 11: Conformance test suite

**Crate**: `notsecrets`
**File(s)**: `crates/notsecrets/tests/conformance_secret_source.rs`
**Run**: `cargo nextest run -p notsecrets -- conformance`

1. Create conformance test:

   ```rust
   use notsecrets::ports::{SecretSource, EnumerableSecretSource};
   use notsecrets::sources::*;

   fn assert_secret_source_contract(source: &dyn SecretSource) {
       // name is non-empty
       assert!(!source.name().is_empty(), "{}: name() must be non-empty", source.name());

       // resolve for unknown key returns Ok(None)
       let result = source.resolve("__CONFORMANCE_NONEXISTENT_KEY_99999__");
       assert!(
           result.is_ok(),
           "{}: resolve(unknown) should be Ok, got {:?}",
           source.name(),
           result,
       );
       assert_eq!(
           result.unwrap(),
           None,
           "{}: resolve(unknown) should be None",
           source.name(),
       );
   }

   fn assert_enumerable_contract(source: &dyn EnumerableSecretSource) {
       assert_secret_source_contract(source);

       // resolve_all returns Ok
       let map = source.resolve_all();
       assert!(
           map.is_ok(),
           "{}: resolve_all() should be Ok, got {:?}",
           source.name(),
           map,
       );
   }

   #[test]
   fn conformance_env_source() {
       let s = EnvSource;
       assert_enumerable_contract(&s);
   }

   #[test]
   fn conformance_op_source() {
       let s = OpSource::new("test.1password.com".to_string());
       assert_secret_source_contract(&s);
   }

   #[test]
   fn conformance_gsm_source() {
       let s = GsmSource::new("test-project".to_string());
       assert_secret_source_contract(&s);
   }

   #[test]
   fn conformance_nuenv_stub() {
       let s = NuenvSource;
       assert_secret_source_contract(&s);
   }

   #[test]
   fn conformance_direnv_stub() {
       let s = DirenvSource;
       assert_secret_source_contract(&s);
   }

   #[test]
   fn conformance_mise_stub() {
       let s = MiseSource;
       assert_secret_source_contract(&s);
   }

   #[test]
   fn conformance_vault_stub() {
       let s = VaultSource;
       assert_secret_source_contract(&s);
   }

   #[test]
   fn conformance_dotenvy_stub() {
       let s = DotenvySource;
       assert_secret_source_contract(&s);
   }
   ```

   Note: `DotenvxSource` and `SopsSource` are excluded from conformance because
   `resolve(unknown)` triggers a CLI call that may fail without the tool installed.
   They get unit-tested individually.

2. Verify:

   ```
   cargo nextest run -p notsecrets -- conformance  -> all green
   ```

3. Commit: `test(notsecrets): add SecretSource conformance test suite`

---

### Task 12: notstrap integration

**Crate**: `notstrap`
**File(s)**: `crates/notstrap/src/lib.rs`
**Run**: `cargo nextest run -p notstrap`

1. Replace `EnvInjector` type alias and `env_injector` field with
   `SecretResolver` usage. See design doc Section 3 for exact code.

2. Remove `env_injector` from `BootstrapOptions`.

3. Update any tests in notstrap that construct `BootstrapOptions` to
   remove the `env_injector` field.

4. Verify:

   ```
   cargo nextest run -p notstrap           -> all green
   cargo nextest run --workspace           -> all green
   cargo clippy --workspace -- -D warnings -> zero warnings
   ```

5. Commit: `refactor(notstrap): use SecretResolver instead of EnvInjector`

---

### Task 13: Workspace integration test

**Crate**: integration tests
**File(s)**: `tests/integration/tests/cross_crate.rs`
**Run**: `cargo nextest run -p integration`

1. Add a test that constructs a `SecretResolver` from a minimal config
   and resolves an env var end-to-end:

   ```rust
   #[test]
   fn secret_resolver_resolves_env_var_cross_crate() {
       use notsecrets::config::{Provider, SecretsConfig};
       use notsecrets::resolver::SecretResolver;
       use std::collections::HashMap;

       unsafe { std::env::set_var("NOTFILES_CROSS_CRATE_TEST", "works"); }

       let config = SecretsConfig {
           providers: vec![Provider::Env],
           provider: HashMap::new(),
           secrets: HashMap::new(),
       };
       let resolver = SecretResolver::from_config(config).unwrap();
       let val = resolver.resolve("NOTFILES_CROSS_CRATE_TEST").unwrap();
       assert_eq!(val, Some("works".to_string()));

       unsafe { std::env::remove_var("NOTFILES_CROSS_CRATE_TEST"); }
   }
   ```

2. Verify:

   ```
   cargo nextest run --workspace  -> all green
   ```

3. Commit: `test(integration): add SecretResolver cross-crate test`
