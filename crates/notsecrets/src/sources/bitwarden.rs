//! Bitwarden CLI (`bw`) integration.
//!
//! Serves two roles:
//!
//! * [`SecretSource`] -- resolves `{ source = "bitwarden", item = "...", field = "..." }`
//!   bindings from `notsecrets.toml` by shelling out to `bw get`.
//! * [`IdentitySource`] -- loads an age identity stored in the secure note of a
//!   configured vault item (used by `notstrap` during bootstrap).
//!
//! Both paths share a single unlocked vault session, cached for the lifetime of the
//! source so the master password is prompted for at most once per process.

use crate::config::{Provider, SecretRef};
use crate::error::{AgeError, SecretsError};
use crate::identities::Identity;
use crate::identities::x25519::X25519Identity;
use crate::ports::{IdentitySource, SecretSource};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use zeroize::Zeroize;

/// Field name used when a binding does not specify one.
pub const DEFAULT_FIELD: &str = "password";

/// `bw get <object> <id>` sub-commands that print a single value directly.
/// Anything else is looked up as a custom field on the item's JSON.
const NATIVE_OBJECTS: &[&str] = &["password", "username", "uri", "totp", "notes", "exposed"];

/// Vault lock state reported by `bw status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VaultStatus {
    Unauthenticated,
    Locked,
    Unlocked,
}

pub struct BitwardenSource {
    /// Vault item holding the age identity, used by the [`IdentitySource`] path only.
    pub item_name: String,
    /// Expected `serverUrl`. When set, a mismatch against `bw status` is a hard error
    /// rather than silently reading from the wrong (e.g. cloud vs self-hosted) vault.
    server_url: Option<String>,
    /// Cached `BW_SESSION` token, so we unlock at most once per process.
    session: OnceLock<String>,
}

impl BitwardenSource {
    pub fn new(item_name: impl Into<String>) -> Self {
        Self {
            item_name: item_name.into(),
            server_url: None,
            session: OnceLock::new(),
        }
    }

    /// Pin this source to a specific Bitwarden server (self-hosted or cloud).
    pub fn with_server_url(mut self, server_url: Option<String>) -> Self {
        self.server_url = server_url;
        self
    }

    /// Run `bw` with the given args, returning trimmed stdout on success.
    ///
    /// SAFETY: every argument is passed as a discrete `.args()` element -- no shell is
    /// involved -- so user-controlled item names and field names cannot inject commands.
    fn run_bw(args: &[&str]) -> Result<String, anyhow::Error> {
        let output = Command::new("bw").args(args).output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!(
                    "bw CLI not found in PATH -- install the Bitwarden CLI \
                     (https://bitwarden.com/help/cli/) to use the bitwarden provider"
                )
            } else {
                anyhow::anyhow!("failed to run bw: {e}")
            }
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "bw {} failed: {}",
                args.first().copied().unwrap_or(""),
                stderr.trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Parse `bw status` JSON into a lock state, checking the server URL if one is pinned.
    fn vault_status(&self) -> Result<VaultStatus, anyhow::Error> {
        // `bw status` prints JSON on its own; parse_status_json tolerates any
        // leading warning lines, so no output flags are needed here.
        let raw = Self::run_bw(&["status"])?;
        let json: serde_json::Value = parse_status_json(&raw)?;

        if let Some(expected) = &self.server_url {
            let actual = json
                .get("serverUrl")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim_end_matches('/');
            let expected_trimmed = expected.trim_end_matches('/');
            // An empty serverUrl means the official cloud vault.
            let actual = if actual.is_empty() {
                "https://vault.bitwarden.com"
            } else {
                actual
            };
            if actual != expected_trimmed {
                return Err(anyhow::anyhow!(
                    "bw is logged in to '{actual}' but notsecrets.toml pins \
                     server_url = '{expected_trimmed}' -- run `bw config server \
                     {expected_trimmed}` and log in again"
                ));
            }
        }

        Ok(match json.get("status").and_then(|v| v.as_str()) {
            Some("unlocked") => VaultStatus::Unlocked,
            Some("locked") => VaultStatus::Locked,
            _ => VaultStatus::Unauthenticated,
        })
    }

    /// Return a usable session token, unlocking the vault (prompting) if necessary.
    ///
    /// Order: cached token -> `$BW_SESSION` -> already-unlocked vault -> `bw unlock`.
    fn session(&self) -> Result<String, anyhow::Error> {
        if let Some(cached) = self.session.get() {
            return Ok(cached.clone());
        }

        let session = self.acquire_session()?;
        // A concurrent unlock may have won the race; either token is valid.
        let _ = self.session.set(session.clone());
        Ok(session)
    }

    fn acquire_session(&self) -> Result<String, anyhow::Error> {
        let env_session = std::env::var("BW_SESSION").unwrap_or_default();
        if !env_session.is_empty() {
            // Still validate the server pin before handing the token out.
            self.vault_status()?;
            return Ok(env_session);
        }

        match self.vault_status()? {
            VaultStatus::Unauthenticated => {
                return Err(anyhow::anyhow!(
                    "bw is not logged in -- run `bw login` first"
                ));
            }
            // An already-unlocked vault still needs a session token for scripted use,
            // so fall through to `bw unlock` either way.
            VaultStatus::Locked | VaultStatus::Unlocked => {}
        }

        let mut password = rpassword::prompt_password("Bitwarden master password: ")
            .map_err(|e| anyhow::anyhow!("could not read master password: {e}"))?;

        // Pass the password over stdin so it never appears in the process list.
        let mut child = Command::new("bw")
            .args(["unlock", "--raw", "--passwordstdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("bw unlock spawn: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            let write_result = stdin.write_all(password.as_bytes());
            drop(stdin);
            write_result.map_err(|e| anyhow::anyhow!("bw unlock stdin write: {e}"))?;
        }
        password.zeroize();

        let output = child
            .wait_with_output()
            .map_err(|e| anyhow::anyhow!("bw unlock wait: {e}"))?;
        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "bw unlock failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let session = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if session.is_empty() {
            return Err(anyhow::anyhow!("bw unlock returned an empty session token"));
        }
        Ok(session)
    }

    /// Read one field of a vault item.
    ///
    /// Native objects (`password`, `username`, ...) go through `bw get <object> <item>`.
    /// Any other name is treated as a custom field and read out of the item JSON.
    fn get_field(&self, item: &str, field: &str) -> Result<Option<String>, anyhow::Error> {
        let session = self.session()?;

        if is_native_object(field) {
            let value =
                Self::run_bw(&["get", field, item, "--session", &session, "--nointeraction"])?;
            return Ok(if value.is_empty() { None } else { Some(value) });
        }

        let raw = Self::run_bw(&[
            "get",
            "item",
            item,
            "--session",
            &session,
            "--nointeraction",
        ])?;
        extract_custom_field(&raw, field)
    }
}

/// `bw status` may print warnings before the JSON body; take the object that follows.
fn parse_status_json(raw: &str) -> Result<serde_json::Value, anyhow::Error> {
    let start = raw
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("bw status returned no JSON object: {raw}"))?;
    serde_json::from_str(&raw[start..])
        .map_err(|e| anyhow::anyhow!("could not parse bw status JSON: {e}"))
}

fn is_native_object(field: &str) -> bool {
    NATIVE_OBJECTS.contains(&field)
}

/// Pull a custom field's value out of `bw get item` JSON.
///
/// Custom field names are matched case-insensitively, mirroring how the Bitwarden
/// apps treat them. Returns `Ok(None)` when the item has no such field.
fn extract_custom_field(item_json: &str, field: &str) -> Result<Option<String>, anyhow::Error> {
    let json: serde_json::Value = serde_json::from_str(item_json)
        .map_err(|e| anyhow::anyhow!("could not parse bw item JSON: {e}"))?;

    let Some(fields) = json.get("fields").and_then(|v| v.as_array()) else {
        return Ok(None);
    };

    for entry in fields {
        let name = entry.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.eq_ignore_ascii_case(field) {
            return Ok(entry
                .get("value")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty()));
        }
    }

    Ok(None)
}

impl SecretSource for BitwardenSource {
    fn name(&self) -> &str {
        "bitwarden"
    }

    fn provider(&self) -> Provider {
        Provider::Bitwarden
    }

    /// Bitwarden cannot resolve a bare key name -- an explicit binding naming the
    /// vault item is required, so the priority chain always falls through.
    fn resolve(&self, _key: &str) -> Result<Option<String>, SecretsError> {
        Ok(None)
    }

    fn resolve_ref(
        &self,
        key: &str,
        secret_ref: &SecretRef,
    ) -> Result<Option<String>, SecretsError> {
        let SecretRef::Bitwarden { item, field } = secret_ref else {
            return self.resolve(key);
        };
        let field = field.as_deref().unwrap_or(DEFAULT_FIELD);
        self.get_field(item, field)
            .map_err(|source| SecretsError::SourceError {
                name: "bitwarden".to_string(),
                source,
            })
    }
}

impl IdentitySource for BitwardenSource {
    fn name(&self) -> &str {
        "bitwarden"
    }

    fn load(&self) -> Result<Box<dyn Identity>, AgeError> {
        let key = self
            .get_field(&self.item_name, "notes")
            .map_err(|source| AgeError::SourceError {
                name: "bitwarden".to_string(),
                source,
            })?
            .ok_or_else(|| AgeError::SourceError {
                name: "bitwarden".to_string(),
                source: anyhow::anyhow!("Bitwarden item '{}' has empty notes", self.item_name),
            })?;

        let identity =
            X25519Identity::from_bech32(key.trim()).map_err(|e| AgeError::SourceError {
                name: "bitwarden".to_string(),
                source: anyhow::anyhow!("key is not a valid AGE-SECRET-KEY-1: {e}"),
            })?;
        Ok(Box::new(identity))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_and_provider() {
        let source = BitwardenSource::new("age-key");
        assert_eq!(SecretSource::name(&source), "bitwarden");
        assert_eq!(source.provider(), Provider::Bitwarden);
    }

    #[test]
    fn resolve_without_ref_returns_none() {
        let source = BitwardenSource::new("age-key");
        assert_eq!(source.resolve("SOME_KEY").unwrap(), None);
    }

    #[test]
    fn resolve_ref_wrong_variant_delegates() {
        let source = BitwardenSource::new("age-key");
        assert_eq!(source.resolve_ref("KEY", &SecretRef::Env).unwrap(), None);
    }

    #[test]
    fn native_objects_recognized() {
        for field in ["password", "username", "uri", "totp", "notes", "exposed"] {
            assert!(is_native_object(field), "{field} should be native");
        }
        assert!(!is_native_object("api_key"));
        assert!(!is_native_object("Password")); // bw sub-commands are lowercase
    }

    #[test]
    fn default_field_is_password() {
        assert_eq!(DEFAULT_FIELD, "password");
    }

    #[test]
    fn extract_custom_field_finds_value() {
        let json = r#"{"name":"db","fields":[{"name":"api_key","value":"sk-123","type":1}]}"#;
        assert_eq!(
            extract_custom_field(json, "api_key").unwrap(),
            Some("sk-123".to_string())
        );
    }

    #[test]
    fn extract_custom_field_is_case_insensitive() {
        let json = r#"{"fields":[{"name":"API_Key","value":"sk-123"}]}"#;
        assert_eq!(
            extract_custom_field(json, "api_key").unwrap(),
            Some("sk-123".to_string())
        );
    }

    #[test]
    fn extract_custom_field_missing_returns_none() {
        let json = r#"{"fields":[{"name":"other","value":"x"}]}"#;
        assert_eq!(extract_custom_field(json, "api_key").unwrap(), None);
    }

    #[test]
    fn extract_custom_field_no_fields_array_returns_none() {
        let json = r#"{"name":"db","login":{"password":"p"}}"#;
        assert_eq!(extract_custom_field(json, "api_key").unwrap(), None);
    }

    #[test]
    fn extract_custom_field_empty_value_is_none() {
        let json = r#"{"fields":[{"name":"api_key","value":""}]}"#;
        assert_eq!(extract_custom_field(json, "api_key").unwrap(), None);
    }

    #[test]
    fn extract_custom_field_bad_json_errors() {
        assert!(extract_custom_field("not json", "api_key").is_err());
    }

    #[test]
    fn parse_status_json_skips_leading_noise() {
        let raw = "warning: something\n{\"status\":\"locked\",\"serverUrl\":null}";
        let json = parse_status_json(raw).unwrap();
        assert_eq!(json.get("status").unwrap().as_str(), Some("locked"));
    }

    #[test]
    fn parse_status_json_without_object_errors() {
        assert!(parse_status_json("no json here").is_err());
    }

    #[test]
    fn with_server_url_sets_pin() {
        let source = BitwardenSource::new("age-key")
            .with_server_url(Some("https://vault.example.com".to_string()));
        assert_eq!(
            source.server_url.as_deref(),
            Some("https://vault.example.com")
        );
    }
}
