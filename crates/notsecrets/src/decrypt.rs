use crate::error::AgeError;
use crate::format::{header_bytes_up_to_footer, parse_header};
use crate::identities::{FileKey, Identity};
use chacha20poly1305::{
    aead::Aead,
    ChaCha20Poly1305, Key, KeyInit, Nonce,
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub struct Decryptor {
    identities: Vec<Box<dyn Identity>>,
}

impl Decryptor {
    pub fn with_identities(identities: Vec<Box<dyn Identity>>) -> Self {
        Self { identities }
    }

    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, AgeError> {
        let (header, payload_offset) = parse_header(ciphertext)?;

        // Try to recover file key: try each identity against each stanza.
        // `None` means the stanza type doesn't match; `Some(Err)` means the
        // type matched but key material didn't — treat both as "keep trying".
        let mut file_key: Option<FileKey> = None;
        'outer: for identity in &self.identities {
            for stanza in &header.recipients {
                if let Some(Ok(fk)) = identity.unwrap_file_key(stanza) {
                    file_key = Some(fk);
                    break 'outer;
                }
            }
        }
        let file_key = file_key.ok_or(AgeError::NoMatch)?;

        // Verify header MAC
        let mac_key = derive_mac_key(&file_key)?;
        let header_input = header_bytes_up_to_footer(ciphertext)?;
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&mac_key)
            .map_err(|e| AgeError::CryptoError(format!("HMAC init: {e}")))?;
        mac.update(&header_input);
        mac.verify_slice(&header.mac)
            .map_err(|_| AgeError::MacMismatch)?;

        // Decrypt payload
        let payload = &ciphertext[payload_offset..];
        if payload.len() < 16 {
            return Err(AgeError::ParseError(
                "payload too short to contain nonce".to_string(),
            ));
        }
        let nonce_bytes: [u8; 16] = payload[..16].try_into().expect("slice is exactly 16 bytes");
        let payload_ciphertext = &payload[16..];

        let payload_key = derive_payload_key(&file_key, &nonce_bytes)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&payload_key));
        let counter_nonce = Nonce::default();
        let plaintext = cipher
            .decrypt(&counter_nonce, payload_ciphertext)
            .map_err(|_| AgeError::CryptoError("payload decryption failed".to_string()))?;

        Ok(plaintext)
    }
}

fn derive_mac_key(file_key: &FileKey) -> Result<[u8; 32], AgeError> {
    let hk = Hkdf::<Sha256>::new(Some(b""), file_key.as_bytes());
    let mut mac_key = [0u8; 32];
    hk.expand(b"header", &mut mac_key)
        .map_err(|e| AgeError::CryptoError(format!("HKDF mac key: {e}")))?;
    Ok(mac_key)
}

fn derive_payload_key(file_key: &FileKey, nonce: &[u8; 16]) -> Result<[u8; 32], AgeError> {
    let hk = Hkdf::<Sha256>::new(Some(nonce.as_slice()), file_key.as_bytes());
    let mut payload_key = [0u8; 32];
    hk.expand(b"payload", &mut payload_key)
        .map_err(|e| AgeError::CryptoError(format!("HKDF payload key: {e}")))?;
    Ok(payload_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identities::x25519::X25519Identity;
    use crate::recipients::x25519::X25519Recipient;
    use crate::Encryptor;
    use rand::rngs::OsRng;
    use x25519_dalek::{PublicKey, StaticSecret};

    fn make_x25519_pair() -> (X25519Identity, X25519Recipient) {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        (
            X25519Identity::from_static_secret(secret),
            X25519Recipient::from_public_key(public),
        )
    }

    #[test]
    fn decryptor_roundtrip_single_recipient() {
        let (identity, recipient) = make_x25519_pair();
        let encryptor = Encryptor::with_recipients(vec![Box::new(recipient)]).unwrap();
        let plaintext = b"the quick brown fox";
        let ciphertext = encryptor.encrypt(plaintext).unwrap();
        let decryptor = Decryptor::with_identities(vec![Box::new(identity)]);
        let decrypted = decryptor.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decryptor_roundtrip_multiple_recipients() {
        let (identity1, recipient1) = make_x25519_pair();
        let (identity2, recipient2) = make_x25519_pair();
        let encryptor = Encryptor::with_recipients(vec![
            Box::new(recipient1),
            Box::new(recipient2),
        ])
        .unwrap();
        let plaintext = b"multi-recipient test";
        let ciphertext = encryptor.encrypt(plaintext).unwrap();
        let decryptor1 = Decryptor::with_identities(vec![Box::new(identity1)]);
        assert_eq!(decryptor1.decrypt(&ciphertext).unwrap(), plaintext);
        let decryptor2 = Decryptor::with_identities(vec![Box::new(identity2)]);
        assert_eq!(decryptor2.decrypt(&ciphertext).unwrap(), plaintext);
    }

    #[test]
    fn decryptor_no_matching_identity_returns_no_match() {
        let (_, recipient) = make_x25519_pair();
        let (wrong_identity, _) = make_x25519_pair();
        let encryptor = Encryptor::with_recipients(vec![Box::new(recipient)]).unwrap();
        let ciphertext = encryptor.encrypt(b"data").unwrap();
        let decryptor = Decryptor::with_identities(vec![Box::new(wrong_identity)]);
        let err = decryptor.decrypt(&ciphertext).unwrap_err();
        assert!(matches!(err, crate::error::AgeError::NoMatch));
    }

    #[test]
    fn decryptor_corrupted_mac_returns_mac_mismatch() {
        let (identity, recipient) = make_x25519_pair();
        let encryptor = Encryptor::with_recipients(vec![Box::new(recipient)]).unwrap();
        let mut ciphertext = encryptor.encrypt(b"data").unwrap();
        // Flip a byte in the MAC region (after "--- ")
        let footer_pos = ciphertext
            .windows(4)
            .position(|w| w == b"--- ")
            .expect("footer present");
        ciphertext[footer_pos + 4] ^= 0xff;
        let decryptor = Decryptor::with_identities(vec![Box::new(identity)]);
        let err = decryptor.decrypt(&ciphertext).unwrap_err();
        assert!(
            matches!(
                err,
                crate::error::AgeError::MacMismatch | crate::error::AgeError::ParseError(_)
            ),
            "expected MacMismatch or ParseError, got: {err:?}"
        );
    }
}
