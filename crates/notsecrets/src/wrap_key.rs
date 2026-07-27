use crate::error::AgeError;
use hkdf::Hkdf;
use sha2::Sha256;

/// Derive the 32-byte ChaCha20-Poly1305 wrap key shared by all age X25519-based
/// stanza types (`X25519`, `ssh-ed25519`) via HKDF-SHA256 over the DH shared
/// secret, ephemeral public key, and recipient public key, salted by a
/// stanza-type-specific HKDF info string.
pub(crate) fn derive_wrap_key(
    shared: &[u8],
    ephemeral_pub: &[u8],
    recipient_pub: &[u8],
    info: &[u8],
) -> Result<[u8; 32], AgeError> {
    let mut ikm = Vec::with_capacity(shared.len() + ephemeral_pub.len() + recipient_pub.len());
    ikm.extend_from_slice(shared);
    ikm.extend_from_slice(ephemeral_pub);
    ikm.extend_from_slice(recipient_pub);

    let hk = Hkdf::<Sha256>::new(None, &ikm);
    let mut wrap_key = [0u8; 32];
    hk.expand(info, &mut wrap_key)
        .map_err(|e| AgeError::CryptoError(format!("HKDF expand: {e}")))?;
    Ok(wrap_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_wrap_key_is_deterministic_and_info_dependent() {
        let shared = [1u8; 32];
        let ephemeral_pub = [2u8; 32];
        let recipient_pub = [3u8; 32];

        let a = derive_wrap_key(&shared, &ephemeral_pub, &recipient_pub, b"info-a").unwrap();
        let a_again = derive_wrap_key(&shared, &ephemeral_pub, &recipient_pub, b"info-a").unwrap();
        let b = derive_wrap_key(&shared, &ephemeral_pub, &recipient_pub, b"info-b").unwrap();

        assert_eq!(a, a_again);
        assert_ne!(a, b);
    }
}
