use crate::bech32_util::decode_bech32_32;
use crate::error::AgeError;
use crate::identities::x25519::HKDF_INFO;
use crate::identities::{FileKey, Stanza};
use crate::recipients::Recipient;
use crate::wrap_key::derive_wrap_key;
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use bech32::{Bech32, Hrp};
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce, aead::Aead};
use rand::rngs::OsRng;
use x25519_dalek::{EphemeralSecret, PublicKey};

const TAG: &str = "X25519";
const RECIPIENT_HRP: &str = "age";

pub struct X25519Recipient {
    public_key: PublicKey,
}

impl X25519Recipient {
    pub fn from_public_key(public_key: PublicKey) -> Self {
        Self { public_key }
    }

    pub fn from_bech32(s: &str) -> Result<Self, AgeError> {
        let bytes = decode_bech32_32(s, RECIPIENT_HRP, false)?;
        Ok(Self {
            public_key: PublicKey::from(bytes),
        })
    }

    pub fn to_bech32(&self) -> String {
        let hrp = Hrp::parse(RECIPIENT_HRP).expect("static hrp is valid");
        bech32::encode::<Bech32>(hrp, self.public_key.as_bytes())
            .expect("bech32 encode cannot fail for valid input")
    }
}

impl Recipient for X25519Recipient {
    fn wrap_file_key(&self, file_key: &FileKey) -> Result<Stanza, AgeError> {
        let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
        let ephemeral_pub = PublicKey::from(&ephemeral_secret);
        let shared = ephemeral_secret.diffie_hellman(&self.public_key);

        let wrap_key = derive_wrap_key(
            shared.as_bytes(),
            ephemeral_pub.as_bytes(),
            self.public_key.as_bytes(),
            HKDF_INFO,
        )?;

        let cipher = ChaCha20Poly1305::new(Key::from_slice(&wrap_key));
        let nonce = Nonce::default();
        let wrapped = cipher
            .encrypt(&nonce, file_key.as_bytes().as_slice())
            .map_err(|_| AgeError::CryptoError("X25519 file key encryption failed".to_string()))?;

        Ok(Stanza {
            tag: TAG.to_string(),
            args: vec![STANDARD_NO_PAD.encode(ephemeral_pub.as_bytes())],
            body: wrapped,
        })
    }
}
