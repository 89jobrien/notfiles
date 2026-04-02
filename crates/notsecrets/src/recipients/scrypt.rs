use crate::error::AgeError;
use crate::identities::{FileKey, Stanza};
use crate::identities::scrypt::derive_scrypt_key;
use crate::recipients::Recipient;
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce, aead::Aead};
use rand::RngCore;

const TAG: &str = "scrypt";
const SALT_LEN: usize = 16;

pub struct ScryptRecipient {
    passphrase: String,
    work_factor: u8,
}

impl ScryptRecipient {
    /// `work_factor` is log2(N). Use 18 in production, 14 in tests.
    pub fn new(passphrase: String, work_factor: u8) -> Self {
        Self { passphrase, work_factor }
    }
}

impl Recipient for ScryptRecipient {
    fn wrap_file_key(&self, file_key: &FileKey) -> Result<Stanza, AgeError> {
        let mut salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);

        let wrap_key = derive_scrypt_key(self.passphrase.as_bytes(), &salt, self.work_factor)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&wrap_key));
        let nonce = Nonce::default();
        let wrapped = cipher
            .encrypt(&nonce, file_key.as_bytes().as_slice())
            .map_err(|_| AgeError::CryptoError("scrypt file key encryption failed".to_string()))?;

        Ok(Stanza {
            tag: TAG.to_string(),
            args: vec![
                STANDARD_NO_PAD.encode(salt),
                self.work_factor.to_string(),
            ],
            body: wrapped,
        })
    }
}
