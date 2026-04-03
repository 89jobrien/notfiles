use crate::error::AgeError;
use crate::identities::{FileKey, Identity, Stanza};
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce, aead::Aead};

const TAG: &str = "scrypt";

pub struct ScryptIdentity {
    passphrase: String,
}

impl ScryptIdentity {
    pub fn new(passphrase: String) -> Self {
        Self { passphrase }
    }
}

impl Identity for ScryptIdentity {
    fn unwrap_file_key(&self, stanza: &Stanza) -> Option<Result<FileKey, AgeError>> {
        if stanza.tag != TAG {
            return None;
        }
        Some(unwrap(stanza, &self.passphrase))
    }
}

fn unwrap(stanza: &Stanza, passphrase: &str) -> Result<FileKey, AgeError> {
    if stanza.args.len() != 2 {
        return Err(AgeError::ParseError(
            "scrypt stanza must have 2 args: salt work_factor".to_string(),
        ));
    }
    let salt = STANDARD_NO_PAD
        .decode(&stanza.args[0])
        .map_err(|e| AgeError::ParseError(format!("scrypt salt base64: {e}")))?;
    let work_factor: u8 = stanza.args[1]
        .parse()
        .map_err(|e| AgeError::ParseError(format!("scrypt work factor parse: {e}")))?;

    let wrap_key = derive_scrypt_key(passphrase.as_bytes(), &salt, work_factor)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&wrap_key));
    let nonce = Nonce::default();
    let file_key_bytes = cipher
        .decrypt(&nonce, stanza.body.as_slice())
        .map_err(|_| AgeError::CryptoError("scrypt file key decryption failed".to_string()))?;
    FileKey::try_from(file_key_bytes.as_slice())
}

pub(crate) fn derive_scrypt_key(
    passphrase: &[u8],
    salt: &[u8],
    work_factor: u8,
) -> Result<[u8; 32], AgeError> {
    let params = scrypt::Params::new(work_factor, 8, 1, 32)
        .map_err(|e| AgeError::CryptoError(format!("scrypt params: {e}")))?;
    let mut wrap_key = [0u8; 32];
    scrypt::scrypt(passphrase, salt, &params, &mut wrap_key)
        .map_err(|e| AgeError::CryptoError(format!("scrypt: {e}")))?;
    Ok(wrap_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identities::FileKey;
    use crate::recipients::Recipient;
    use crate::recipients::scrypt::ScryptRecipient;

    #[test]
    fn scrypt_wrap_unwrap_roundtrip() {
        let passphrase = "hunter2".to_string();
        let recipient = ScryptRecipient::new(passphrase.clone(), 14);
        let identity = ScryptIdentity::new(passphrase);

        let file_key = FileKey::new([0x11u8; 16]);
        let stanza = recipient
            .wrap_file_key(&file_key)
            .expect("wrap should succeed");
        assert_eq!(stanza.tag, "scrypt");
        assert_eq!(stanza.args.len(), 2);

        let unwrapped = identity
            .unwrap_file_key(&stanza)
            .expect("identity should match")
            .expect("unwrap should succeed");
        assert_eq!(unwrapped.as_bytes(), file_key.as_bytes());
    }

    #[test]
    fn scrypt_wrong_passphrase_returns_crypto_error() {
        let recipient = ScryptRecipient::new("correct".to_string(), 14);
        let identity = ScryptIdentity::new("wrong".to_string());

        let file_key = FileKey::new([0x11u8; 16]);
        let stanza = recipient.wrap_file_key(&file_key).unwrap();

        let result = identity
            .unwrap_file_key(&stanza)
            .expect("tag matches")
            .expect_err("wrong passphrase should fail");
        assert!(matches!(result, crate::error::AgeError::CryptoError(_)));
    }

    #[test]
    fn scrypt_wrong_tag_returns_none() {
        let identity = ScryptIdentity::new("pass".to_string());
        let stanza = Stanza {
            tag: "X25519".to_string(),
            args: vec!["arg".to_string()],
            body: vec![],
        };
        assert!(identity.unwrap_file_key(&stanza).is_none());
    }
}
