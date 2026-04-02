use crate::error::AgeError;
use crate::identities::ssh_ed25519::{derive_wrap_key, ssh_key_fingerprint};
use crate::identities::{FileKey, Stanza};
use crate::recipients::Recipient;
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce, aead::Aead};
use ed25519_dalek::VerifyingKey;
use rand::rngs::OsRng;
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey};

const TAG: &str = "ssh-ed25519";

pub struct SshEd25519Recipient {
    verifying_key: VerifyingKey,
}

impl SshEd25519Recipient {
    pub fn from_verifying_key(verifying_key: VerifyingKey) -> Self {
        Self { verifying_key }
    }

    fn x25519_public(&self) -> X25519PublicKey {
        use curve25519_dalek::edwards::CompressedEdwardsY;
        let compressed = CompressedEdwardsY(self.verifying_key.to_bytes());
        let point = compressed
            .decompress()
            .expect("valid ed25519 pubkey decompresses");
        X25519PublicKey::from(point.to_montgomery().to_bytes())
    }
}

impl Recipient for SshEd25519Recipient {
    fn wrap_file_key(&self, file_key: &FileKey) -> Result<Stanza, AgeError> {
        let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
        let ephemeral_pub = X25519PublicKey::from(&ephemeral_secret);
        let recipient_x25519 = self.x25519_public();
        let shared = ephemeral_secret.diffie_hellman(&recipient_x25519);

        let wrap_key = derive_wrap_key(
            shared.as_bytes(),
            ephemeral_pub.as_bytes(),
            recipient_x25519.as_bytes(),
        )?;

        let cipher = ChaCha20Poly1305::new(Key::from_slice(&wrap_key));
        let nonce = Nonce::default();
        let wrapped = cipher
            .encrypt(&nonce, file_key.as_bytes().as_slice())
            .map_err(|_| {
                AgeError::CryptoError("ssh-ed25519 file key encryption failed".to_string())
            })?;

        let fingerprint = ssh_key_fingerprint(self.verifying_key.as_bytes());

        Ok(Stanza {
            tag: TAG.to_string(),
            args: vec![
                STANDARD_NO_PAD.encode(fingerprint),
                STANDARD_NO_PAD.encode(ephemeral_pub.as_bytes()),
            ],
            body: wrapped,
        })
    }
}
