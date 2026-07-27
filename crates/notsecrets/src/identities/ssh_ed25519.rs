use crate::error::AgeError;
use crate::identities::{FileKey, Identity, Stanza};
use crate::wrap_key::derive_wrap_key;
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce, aead::Aead};
use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256, Sha512};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519StaticSecret};

const TAG: &str = "ssh-ed25519";
pub(crate) const HKDF_INFO: &[u8] = b"age-encryption.org/v1/ssh-ed25519";

pub struct SshEd25519Identity {
    signing_key: SigningKey,
}

impl SshEd25519Identity {
    pub fn from_signing_key(signing_key: SigningKey) -> Self {
        Self { signing_key }
    }

    fn x25519_secret(&self) -> X25519StaticSecret {
        let hash = Sha512::digest(self.signing_key.as_bytes());
        let mut scalar = [0u8; 32];
        scalar.copy_from_slice(&hash[..32]);
        scalar[0] &= 248;
        scalar[31] &= 127;
        scalar[31] |= 64;
        X25519StaticSecret::from(scalar)
    }

    fn fingerprint(&self) -> [u8; 4] {
        ssh_key_fingerprint(self.signing_key.verifying_key().as_bytes())
    }
}

impl Identity for SshEd25519Identity {
    fn unwrap_file_key(&self, stanza: &Stanza) -> Option<Result<FileKey, AgeError>> {
        if stanza.tag != TAG {
            return None;
        }
        if stanza.args.len() != 2 {
            return Some(Err(AgeError::ParseError(
                "ssh-ed25519 stanza must have 2 args".to_string(),
            )));
        }
        let fp_bytes = match STANDARD_NO_PAD.decode(&stanza.args[0]) {
            Ok(b) => b,
            Err(e) => {
                return Some(Err(AgeError::ParseError(format!(
                    "fingerprint base64: {e}"
                ))));
            }
        };
        if fp_bytes.as_slice() != self.fingerprint() {
            return None;
        }
        Some(unwrap(stanza, self))
    }
}

fn unwrap(stanza: &Stanza, identity: &SshEd25519Identity) -> Result<FileKey, AgeError> {
    let ephemeral_pub_bytes = STANDARD_NO_PAD
        .decode(&stanza.args[1])
        .map_err(|e| AgeError::ParseError(format!("ephemeral pubkey base64: {e}")))?;
    let ephemeral_pub_bytes: [u8; 32] = ephemeral_pub_bytes
        .try_into()
        .map_err(|_| AgeError::ParseError("ephemeral pubkey must be 32 bytes".to_string()))?;
    let ephemeral_pub = X25519PublicKey::from(ephemeral_pub_bytes);

    let x25519_secret = identity.x25519_secret();
    let shared = x25519_secret.diffie_hellman(&ephemeral_pub);
    let x25519_pub = X25519PublicKey::from(&x25519_secret);

    let wrap_key = derive_wrap_key(
        shared.as_bytes(),
        ephemeral_pub.as_bytes(),
        x25519_pub.as_bytes(),
        HKDF_INFO,
    )?;

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&wrap_key));
    let nonce = Nonce::default();
    let file_key_bytes = cipher
        .decrypt(&nonce, stanza.body.as_slice())
        .map_err(|_| AgeError::CryptoError("ssh-ed25519 file key decryption failed".to_string()))?;
    FileKey::try_from(file_key_bytes.as_slice())
}

pub(crate) fn ssh_key_fingerprint(pub_key_bytes: &[u8]) -> [u8; 4] {
    let hash = Sha256::digest(pub_key_bytes);
    hash[..4].try_into().expect("SHA-256 output is 32 bytes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identities::FileKey;
    use crate::recipients::Recipient;
    use crate::recipients::ssh_ed25519::SshEd25519Recipient;

    fn test_signing_key() -> SigningKey {
        use rand::rngs::OsRng;
        let mut rng = OsRng;
        let mut seed = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rng, &mut seed);
        SigningKey::from_bytes(&seed)
    }

    #[test]
    fn ssh_ed25519_wrap_unwrap_roundtrip() {
        let signing_key = test_signing_key();
        let verifying_key = signing_key.verifying_key();
        let recipient = SshEd25519Recipient::from_verifying_key(verifying_key);
        let identity = SshEd25519Identity::from_signing_key(signing_key);

        let file_key = FileKey::new([0x33u8; 16]);
        let stanza = recipient
            .wrap_file_key(&file_key)
            .expect("wrap should succeed");
        assert_eq!(stanza.tag, "ssh-ed25519");
        assert_eq!(stanza.args.len(), 2);

        let unwrapped = identity
            .unwrap_file_key(&stanza)
            .expect("identity should match")
            .expect("unwrap should succeed");
        assert_eq!(unwrapped.as_bytes(), file_key.as_bytes());
    }

    #[test]
    fn ssh_ed25519_wrong_fingerprint_returns_none() {
        let signing_key1 = test_signing_key();
        let signing_key2 = test_signing_key();
        let verifying_key1 = signing_key1.verifying_key();
        let recipient = SshEd25519Recipient::from_verifying_key(verifying_key1);
        let identity = SshEd25519Identity::from_signing_key(signing_key2);

        let file_key = FileKey::new([0x33u8; 16]);
        let stanza = recipient.wrap_file_key(&file_key).unwrap();
        assert!(identity.unwrap_file_key(&stanza).is_none());
    }
}
