//! Tailscale auth key resolution chain:
//!
//! 1. `TS_AUTHKEY` environment variable
//! 2. YubiKey PIV slot 9d (when compiled with the `yubikey` feature)
//! 3. Interactive prompt
//!
//! Returns `Err(NotnetError::NoAuthKey)` if all sources are exhausted without a key.

use crate::error::NotnetError;

/// Resolve the Tailscale auth key through the production chain: `TS_AUTHKEY`,
/// then the YubiKey (PIV slot 9d, when the `yubikey` feature is compiled in),
/// then an interactive prompt.
pub fn resolve_auth_key() -> Result<String, NotnetError> {
    resolve_auth_key_with(
        std::env::var("TS_AUTHKEY").ok(),
        read_yubikey_slot_9d,
        prompt_auth_key,
    )
}

/// The auth-key chain over explicit sources, so the precedence is unit-testable
/// without a key.
///
/// Each source is consulted in order, stopping at the first non-empty result:
/// `env_key` (the `TS_AUTHKEY` value), then `yubikey_key` (PIV slot 9d), then
/// `prompt`.
pub fn resolve_auth_key_with(
    env_key: Option<String>,
    yubikey_key: impl FnOnce() -> Option<String>,
    prompt: impl FnOnce() -> Result<String, NotnetError>,
) -> Result<String, NotnetError> {
    // 1. Environment variable
    if let Some(key) = env_key
        && !key.is_empty()
    {
        return Ok(key);
    }

    // 2. YubiKey PIV slot 9d
    if let Some(key) = yubikey_key() {
        return Ok(key);
    }

    // 3. Interactive prompt
    prompt()
}

/// Reads the Tailscale auth key from the YubiKey PIV Key Management slot (0x9d).
///
/// Returns `None` when the YubiKey is unreachable or slot 9d is empty — the
/// caller falls through to the next source.
#[cfg(feature = "yubikey")]
fn read_yubikey_slot_9d() -> Option<String> {
    use yubikey::YubiKey;

    // PIV object ID of the Key Management slot: `yubikey::piv::SlotId::object_id`
    // maps `SlotId::KeyManagement` to `0x005f_c10b`, but that method is
    // `pub(crate)`, so the OID is spelled out here.
    const KEY_MANAGEMENT_OBJECT_ID: u32 = 0x005f_c10b;

    let mut yk = YubiKey::open().ok()?;
    let data = yk.fetch_object(KEY_MANAGEMENT_OBJECT_ID).ok()?;
    let key = String::from_utf8(data.to_vec()).ok()?;
    let key = key.trim().to_string();
    if key.is_empty() { None } else { Some(key) }
}

#[cfg(not(feature = "yubikey"))]
fn read_yubikey_slot_9d() -> Option<String> {
    // Compiled without the `yubikey` feature: no slot is consulted.
    None
}

fn prompt_auth_key() -> Result<String, NotnetError> {
    let key = rpassword::prompt_password("Tailscale auth key: ").map_err(NotnetError::Io)?;
    let key = key.trim().to_string();
    if key.is_empty() {
        Err(NotnetError::NoAuthKey)
    } else {
        Ok(key)
    }
}

/// The production `AuthKeySource`: env var, then YubiKey, then an interactive prompt.
pub struct ChainedAuthKey;

impl crate::ports::AuthKeySource for ChainedAuthKey {
    fn auth_key(&self) -> Result<String, NotnetError> {
        resolve_auth_key()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    type Log = Rc<RefCell<Vec<&'static str>>>;

    fn yubikey_stub(log: &Log, key: Option<&'static str>) -> impl FnOnce() -> Option<String> {
        let log = Rc::clone(log);
        move || {
            log.borrow_mut().push("yubikey");
            key.map(String::from)
        }
    }

    fn prompt_stub(
        log: &Log,
        key: Option<&'static str>,
    ) -> impl FnOnce() -> Result<String, NotnetError> {
        let log = Rc::clone(log);
        move || {
            log.borrow_mut().push("prompt");
            match key {
                Some(k) => Ok(k.to_string()),
                None => Err(NotnetError::NoAuthKey),
            }
        }
    }

    #[test]
    fn env_key_wins_when_set() {
        let log = Log::default();
        let out = resolve_auth_key_with(
            Some("env-key".into()),
            yubikey_stub(&log, Some("yk-key")),
            prompt_stub(&log, Some("prompted")),
        )
        .expect("env key");
        assert_eq!(out, "env-key");
        assert!(log.borrow().is_empty(), "later sources must not run");
    }

    #[test]
    fn empty_env_key_falls_through_to_yubikey() {
        let log = Log::default();
        let out = resolve_auth_key_with(
            Some(String::new()),
            yubikey_stub(&log, Some("yk-key")),
            prompt_stub(&log, Some("prompted")),
        )
        .expect("yubikey key");
        assert_eq!(out, "yk-key");
        assert_eq!(log.borrow().to_vec(), ["yubikey"]);
    }

    #[test]
    fn missing_env_key_uses_yubikey() {
        let log = Log::default();
        let out = resolve_auth_key_with(
            None,
            yubikey_stub(&log, Some("yk-key")),
            prompt_stub(&log, Some("prompted")),
        )
        .expect("yubikey key");
        assert_eq!(out, "yk-key");
        assert_eq!(log.borrow().to_vec(), ["yubikey"]);
    }

    #[test]
    fn prompt_is_used_when_sources_are_exhausted() {
        let log = Log::default();
        let out = resolve_auth_key_with(
            None,
            yubikey_stub(&log, None),
            prompt_stub(&log, Some("prompted")),
        )
        .expect("prompted");
        assert_eq!(out, "prompted");
        assert_eq!(log.borrow().to_vec(), ["yubikey", "prompt"]);
    }

    #[test]
    fn exhausted_sources_are_no_auth_key() {
        let log = Log::default();
        let err = resolve_auth_key_with(
            Some(String::new()),
            yubikey_stub(&log, None),
            prompt_stub(&log, None),
        )
        .expect_err("no key");
        assert!(matches!(err, NotnetError::NoAuthKey));
        assert_eq!(log.borrow().to_vec(), ["yubikey", "prompt"]);
    }
}
