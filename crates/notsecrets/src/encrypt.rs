use crate::error::AgeError;
use crate::format::serialize_header;
use crate::identities::{FileKey, Header};
use crate::recipients::Recipient;
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce, aead::Aead};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub struct Encryptor {
    recipients: Vec<Box<dyn Recipient>>,
}

impl Encryptor {
    /// Construct an Encryptor. Returns AgeError if:
    /// - recipients is empty
    /// - more than one scrypt recipient is present
    pub fn with_recipients(recipients: Vec<Box<dyn Recipient>>) -> Result<Self, AgeError> {
        if recipients.is_empty() {
            return Err(AgeError::CryptoError(
                "at least one recipient is required".to_string(),
            ));
        }
        // Detect scrypt recipients by wrapping a dummy key and checking stanza tag
        let dummy_key = FileKey::new([0u8; 16]);
        let scrypt_count = recipients
            .iter()
            .filter_map(|r| r.wrap_file_key(&dummy_key).ok())
            .filter(|s| s.tag == "scrypt")
            .count();
        if scrypt_count > 1 {
            return Err(AgeError::CryptoError(
                "at most one scrypt recipient is allowed per age file".to_string(),
            ));
        }
        Ok(Self { recipients })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, AgeError> {
        let file_key = FileKey::generate();

        // Wrap file key with each recipient
        let stanzas: Vec<_> = self
            .recipients
            .iter()
            .map(|r| r.wrap_file_key(&file_key))
            .collect::<Result<_, _>>()?;

        // Build header without MAC first
        let header_no_mac = Header {
            recipients: stanzas,
            mac: Vec::new(),
        };

        // Serialize and compute MAC: HMAC-SHA256 over bytes up to and including "--- "
        let header_bytes = serialize_header(&header_no_mac);
        let mac_key = derive_mac_key(&file_key)?;
        let footer_pos = header_bytes
            .windows(4)
            .rposition(|w| w == b"--- ")
            .ok_or_else(|| AgeError::ParseError("serialize_header missing footer".to_string()))?;
        let mac_input = &header_bytes[..footer_pos + 4];

        let mut mac = <HmacSha256 as Mac>::new_from_slice(&mac_key)
            .map_err(|e| AgeError::CryptoError(format!("HMAC init: {e}")))?;
        mac.update(mac_input);
        let mac_bytes = mac.finalize().into_bytes().to_vec();

        // Re-serialize with actual MAC
        let header_with_mac = Header {
            recipients: header_no_mac.recipients,
            mac: mac_bytes,
        };
        let header_bytes = serialize_header(&header_with_mac);

        // Payload: 16-byte random nonce + ChaCha20Poly1305(payload_key, plaintext)
        let mut nonce_bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let payload_key = derive_payload_key(&file_key, &nonce_bytes)?;

        let cipher = ChaCha20Poly1305::new(Key::from_slice(&payload_key));
        let counter_nonce = Nonce::default(); // [0u8; 12]
        let ciphertext = cipher
            .encrypt(&counter_nonce, plaintext)
            .map_err(|_| AgeError::CryptoError("payload encryption failed".to_string()))?;

        let mut output = header_bytes;
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);
        Ok(output)
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
    use rand::rngs::OsRng;
    use x25519_dalek::{PublicKey, StaticSecret};

    #[test]
    fn encryptor_single_x25519_recipient() {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        let recipient: Box<dyn crate::recipients::Recipient> =
            Box::new(X25519Recipient::from_public_key(public));
        let encryptor = Encryptor::with_recipients(vec![recipient]).unwrap();
        let plaintext = b"hello, age!";
        let ciphertext = encryptor
            .encrypt(plaintext)
            .expect("encrypt should succeed");
        assert!(
            ciphertext.starts_with(b"age-encryption.org/v1\n"),
            "output must start with age version line"
        );
    }

    #[test]
    fn encryptor_rejects_multiple_scrypt_recipients() {
        use crate::recipients::scrypt::ScryptRecipient;
        let r1: Box<dyn crate::recipients::Recipient> =
            Box::new(ScryptRecipient::new("pass1".to_string(), 14));
        let r2: Box<dyn crate::recipients::Recipient> =
            Box::new(ScryptRecipient::new("pass2".to_string(), 14));
        let result = Encryptor::with_recipients(vec![r1, r2]);
        assert!(result.is_err(), "two scrypt recipients must be rejected");
    }

    #[test]
    fn encryptor_empty_recipients_returns_error() {
        let result = Encryptor::with_recipients(vec![]);
        assert!(result.is_err(), "empty recipients must be rejected");
    }

    // Suppress unused import warning for X25519Identity which may be used in Task 9
    #[allow(dead_code)]
    fn _use_x25519_identity(_: X25519Identity) {}
}
