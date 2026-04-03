use crate::error::AgeError;
use crate::identities::ssh_rsa::rsa_pubkey_fingerprint;
use crate::identities::{FileKey, Stanza};
use crate::recipients::Recipient;
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use rsa::{Oaep, RsaPublicKey};
use sha2::Sha256;

const TAG: &str = "ssh-rsa";

pub struct SshRsaRecipient {
    public_key: RsaPublicKey,
}

impl SshRsaRecipient {
    pub fn from_public_key(public_key: RsaPublicKey) -> Self {
        Self { public_key }
    }
}

impl Recipient for SshRsaRecipient {
    fn wrap_file_key(&self, file_key: &FileKey) -> Result<Stanza, AgeError> {
        use rand::rngs::OsRng;
        let padding = Oaep::new::<Sha256>();
        let wrapped = self
            .public_key
            .encrypt(&mut OsRng, padding, file_key.as_bytes())
            .map_err(|_| AgeError::CryptoError("RSA-OAEP encrypt failed".to_string()))?;

        let fingerprint = rsa_pubkey_fingerprint(&self.public_key);

        Ok(Stanza {
            tag: TAG.to_string(),
            args: vec![STANDARD_NO_PAD.encode(fingerprint)],
            body: wrapped,
        })
    }
}
