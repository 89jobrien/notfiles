use crate::error::AgeError;
use crate::identities::Identity;
use crate::ports::IdentitySource;

/// Reads an age private key from YubiKey PIV slot 9c (Digital Signature slot).
///
/// Requires the `yubikey` feature to be enabled. When the feature is absent this
/// source is compiled out entirely — the struct still exists for type-level use but
/// `load()` always returns an error.
pub struct YubikeySource;

impl IdentitySource for YubikeySource {
    fn name(&self) -> &str {
        "yubikey-piv-9c"
    }

    fn load(&self) -> Result<Box<dyn Identity>, AgeError> {
        load_impl()
    }
}

#[cfg(feature = "yubikey")]
fn load_impl() -> Result<Box<dyn Identity>, AgeError> {
    use crate::identities::x25519::X25519Identity;
    use yubikey::{YubiKey, piv};

    let mut yk = YubiKey::open().map_err(|e| AgeError::SourceError {
        name: "yubikey-piv-9c".to_string(),
        source: anyhow::anyhow!("cannot open YubiKey: {e}"),
    })?;

    // Slot 9c — Digital Signature slot, used here to store the age private key.
    let data =
        piv::read_object(&mut yk, piv::ObjectId::from(piv::SlotId::Signature)).map_err(|e| {
            AgeError::SourceError {
                name: "yubikey-piv-9c".to_string(),
                source: anyhow::anyhow!("cannot read PIV slot 9c: {e}"),
            }
        })?;

    let key = std::str::from_utf8(&data)
        .map_err(|e| AgeError::SourceError {
            name: "yubikey-piv-9c".to_string(),
            source: anyhow::anyhow!("PIV slot 9c data is not valid UTF-8: {e}"),
        })?
        .trim()
        .to_string();

    if key.is_empty() {
        return Err(AgeError::SourceError {
            name: "yubikey-piv-9c".to_string(),
            source: anyhow::anyhow!("PIV slot 9c is empty"),
        });
    }

    let identity = X25519Identity::from_bech32(&key).map_err(|e| AgeError::SourceError {
        name: "yubikey-piv-9c".to_string(),
        source: anyhow::anyhow!("slot 9c content is not a valid AGE-SECRET-KEY-1: {e}"),
    })?;
    Ok(Box::new(identity))
}

#[cfg(not(feature = "yubikey"))]
fn load_impl() -> Result<Box<dyn Identity>, AgeError> {
    Err(AgeError::SourceError {
        name: "yubikey-piv-9c".to_string(),
        source: anyhow::anyhow!(
            "notsecrets was compiled without the `yubikey` feature; \
             YubikeySource is unavailable"
        ),
    })
}
