/// Auth key resolution chain for Tailscale:
///
/// 1. `TS_AUTHKEY` environment variable
/// 2. YubiKey PIV slot 9d (when compiled with the `yubikey` feature)
/// 3. Interactive prompt
///
/// Returns `Err(NotnetError::NoAuthKey)` if all sources are exhausted without a key.
use crate::error::NotnetError;

pub fn resolve_auth_key() -> Result<String, NotnetError> {
    // 1. Environment variable
    if let Ok(key) = std::env::var("TS_AUTHKEY")
        && !key.is_empty()
    {
        return Ok(key);
    }

    // 2. YubiKey PIV slot 9d
    #[cfg(feature = "yubikey")]
    if let Some(key) = read_yubikey_slot_9d() {
        return Ok(key);
    }

    // 3. Interactive prompt
    prompt_auth_key()
}

#[cfg(feature = "yubikey")]
fn read_yubikey_slot_9d() -> Option<String> {
    use yubikey::{YubiKey, piv};

    let mut yk = YubiKey::open().ok()?;
    // Slot 9d — key management slot, repurposed here for the Tailscale auth key.
    let slot = piv::SlotId::KeyManagement; // 0x9d
    let data = piv::read_object(&mut yk, piv::ObjectId::from(slot)).ok()?;
    let key = String::from_utf8(data.to_vec()).ok()?;
    let key = key.trim().to_string();
    if key.is_empty() { None } else { Some(key) }
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
