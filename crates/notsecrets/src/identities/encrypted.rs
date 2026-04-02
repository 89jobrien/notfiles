use crate::error::AgeError;
use crate::identities::{FileKey, Identity, Stanza};

/// An identity whose key material is stored in an age-encrypted file.
///
/// The outer file is decrypted on each call to `unwrap_file_key` using
/// `passphrase_identity`. The decrypted content must be an `AGE-SECRET-KEY-1...`
/// bech32 string, which is parsed as an `X25519Identity`.
pub struct EncryptedIdentity {
    /// Raw bytes of the age-encrypted identity file.
    encrypted_data: Vec<u8>,
    /// Identity used to decrypt the outer age file (typically `ScryptIdentity`).
    passphrase_identity: Box<dyn Identity>,
}

impl EncryptedIdentity {
    pub fn new(encrypted_data: Vec<u8>, passphrase_identity: Box<dyn Identity>) -> Self {
        Self {
            encrypted_data,
            passphrase_identity,
        }
    }
}

impl Identity for EncryptedIdentity {
    fn unwrap_file_key(&self, _stanza: &Stanza) -> Option<Result<FileKey, AgeError>> {
        // Full implementation in Task 9 after Decryptor is available.
        // The integration test below is marked #[ignore] until then.
        let _ = &self.encrypted_data;
        let _ = self.passphrase_identity.as_ref();
        Some(Err(AgeError::CryptoError(
            "EncryptedIdentity requires Decryptor — implemented in Task 9".to_string(),
        )))
    }
}

// Integration test for EncryptedIdentity lives in tests/integration.rs and is
// activated in Task 9 once Encryptor and Decryptor are available.
