use crate::error::AgeError;
use crate::identities::{FileKey, Identity, Stanza};

/// An identity whose key material is stored in an age-encrypted file.
///
/// The outer file is decrypted on each call to `unwrap_file_key` using a
/// scrypt passphrase identity. The decrypted content must be an
/// `AGE-SECRET-KEY-1...` bech32 string, which is parsed as an `X25519Identity`.
pub struct EncryptedIdentity {
    /// Raw bytes of the age-encrypted identity file.
    encrypted_data: Vec<u8>,
    /// Passphrase used to decrypt the outer age file via `ScryptIdentity`.
    passphrase: String,
}

impl EncryptedIdentity {
    pub fn new(encrypted_data: Vec<u8>, passphrase: String) -> Self {
        Self {
            encrypted_data,
            passphrase,
        }
    }
}

impl Identity for EncryptedIdentity {
    fn unwrap_file_key(&self, stanza: &Stanza) -> Option<Result<FileKey, AgeError>> {
        let decryptor = crate::Decryptor::with_identities(vec![Box::new(
            crate::identities::scrypt::ScryptIdentity::new(self.passphrase.clone()),
        )]);
        let plaintext = match decryptor.decrypt(&self.encrypted_data) {
            Ok(p) => p,
            Err(e) => return Some(Err(e)),
        };
        let key_str = match std::str::from_utf8(&plaintext) {
            Ok(s) => s.trim().to_string(),
            Err(_) => {
                return Some(Err(AgeError::ParseError(
                    "decrypted identity file is not valid UTF-8".to_string(),
                )))
            }
        };
        // Parse inner identity — only X25519 supported for now
        let inner: Box<dyn Identity> = if key_str.starts_with("AGE-SECRET-KEY-1") {
            match crate::identities::x25519::X25519Identity::from_bech32(&key_str) {
                Ok(id) => Box::new(id),
                Err(e) => return Some(Err(e)),
            }
        } else {
            return Some(Err(AgeError::UnsupportedKeyType(
                "encrypted identity file must contain AGE-SECRET-KEY-1".to_string(),
            )));
        };
        inner.unwrap_file_key(stanza)
    }
}
