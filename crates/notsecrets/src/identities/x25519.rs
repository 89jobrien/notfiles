use crate::error::AgeError;
use crate::identities::{FileKey, Identity, Stanza};
use crate::wrap_key::derive_wrap_key;
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use bech32::{Bech32, Hrp};
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce, aead::Aead};
use x25519_dalek::{PublicKey, StaticSecret};

const TAG: &str = "X25519";
pub(crate) const HKDF_INFO: &[u8] = b"age-encryption.org/v1/X25519";
const IDENTITY_HRP: &str = "age-secret-key-";

pub struct X25519Identity {
    secret: StaticSecret,
}

impl X25519Identity {
    pub fn from_static_secret(secret: StaticSecret) -> Self {
        Self { secret }
    }

    pub fn from_bech32(s: &str) -> Result<Self, AgeError> {
        let s_lower = s.to_lowercase();
        let (hrp, data) = bech32::decode(&s_lower)
            .map_err(|e| AgeError::ParseError(format!("bech32 decode identity: {e}")))?;
        if hrp.as_str() != IDENTITY_HRP {
            return Err(AgeError::ParseError(format!(
                "expected hrp '{IDENTITY_HRP}', got '{}'",
                hrp.as_str()
            )));
        }
        let bytes: [u8; 32] = data
            .try_into()
            .map_err(|_| AgeError::ParseError("identity key must be 32 bytes".to_string()))?;
        Ok(Self {
            secret: StaticSecret::from(bytes),
        })
    }

    pub fn to_bech32(&self) -> String {
        let hrp = Hrp::parse(IDENTITY_HRP).expect("static hrp is valid");
        bech32::encode::<Bech32>(hrp, self.secret.as_bytes())
            .expect("bech32 encode cannot fail for valid input")
            .to_uppercase()
    }
}

impl Identity for X25519Identity {
    fn unwrap_file_key(&self, stanza: &Stanza) -> Option<Result<FileKey, AgeError>> {
        if stanza.tag != TAG {
            return None;
        }
        Some(unwrap_stanza(stanza, &self.secret))
    }
}

fn unwrap_stanza(stanza: &Stanza, secret: &StaticSecret) -> Result<FileKey, AgeError> {
    if stanza.args.len() != 1 {
        return Err(AgeError::ParseError(
            "X25519 stanza must have exactly 1 arg".to_string(),
        ));
    }
    let ephemeral_pub_bytes = STANDARD_NO_PAD
        .decode(&stanza.args[0])
        .map_err(|e| AgeError::ParseError(format!("ephemeral pubkey base64: {e}")))?;
    let ephemeral_pub_bytes: [u8; 32] = ephemeral_pub_bytes
        .try_into()
        .map_err(|_| AgeError::ParseError("ephemeral pubkey must be 32 bytes".to_string()))?;
    let ephemeral_pub = PublicKey::from(ephemeral_pub_bytes);

    let shared = secret.diffie_hellman(&ephemeral_pub);
    let recipient_pub = PublicKey::from(secret);

    let wrap_key = derive_wrap_key(
        shared.as_bytes(),
        ephemeral_pub.as_bytes(),
        recipient_pub.as_bytes(),
        HKDF_INFO,
    )?;

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&wrap_key));
    let nonce = Nonce::default();
    let file_key_bytes = cipher
        .decrypt(&nonce, stanza.body.as_slice())
        .map_err(|_| AgeError::CryptoError("X25519 file key decryption failed".to_string()))?;

    FileKey::try_from(file_key_bytes.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identities::FileKey;
    use crate::recipients::Recipient;
    use crate::recipients::x25519::X25519Recipient;

    #[test]
    fn x25519_wrap_unwrap_roundtrip() {
        let secret_bytes = [1u8; 32];
        let secret = StaticSecret::from(secret_bytes);
        let public = PublicKey::from(&secret);
        let recipient = X25519Recipient::from_public_key(public);
        let identity = X25519Identity::from_static_secret(StaticSecret::from(secret_bytes));
        let file_key = FileKey::new([0x42u8; 16]);
        let stanza = recipient
            .wrap_file_key(&file_key)
            .expect("wrap should succeed");
        assert_eq!(stanza.tag, "X25519");
        assert_eq!(stanza.args.len(), 1);
        let unwrapped = identity
            .unwrap_file_key(&stanza)
            .expect("identity should match")
            .expect("unwrap should succeed");
        assert_eq!(unwrapped.as_bytes(), file_key.as_bytes());
    }

    #[test]
    fn x25519_wrong_tag_returns_none() {
        let secret = StaticSecret::from([2u8; 32]);
        let identity = X25519Identity::from_static_secret(secret);
        let stanza = Stanza {
            tag: "scrypt".to_string(),
            args: vec!["salt".to_string(), "18".to_string()],
            body: vec![0u8; 32],
        };
        assert!(identity.unwrap_file_key(&stanza).is_none());
    }

    #[test]
    fn x25519_bech32_roundtrip() {
        let secret_bytes = [3u8; 32];
        let secret = StaticSecret::from(secret_bytes);
        let public = PublicKey::from(&secret);
        let recipient_str = X25519Recipient::from_public_key(public).to_bech32();
        assert!(recipient_str.starts_with("age1"));
        let identity_str =
            X25519Identity::from_static_secret(StaticSecret::from(secret_bytes)).to_bech32();
        assert!(identity_str.starts_with("AGE-SECRET-KEY-1"));
    }

    #[test]
    fn x25519_bech32_identity_roundtrip() {
        let secret_bytes = [4u8; 32];
        let identity = X25519Identity::from_static_secret(StaticSecret::from(secret_bytes));
        let encoded = identity.to_bech32();
        let decoded = X25519Identity::from_bech32(&encoded).expect("decode should succeed");
        assert_eq!(decoded.secret.as_bytes(), &secret_bytes);
    }
}
