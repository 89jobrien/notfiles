use crate::error::AgeError;
use crate::identities::{FileKey, Identity, Stanza};
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use rsa::{Oaep, RsaPrivateKey, RsaPublicKey, traits::PublicKeyParts};
use sha2::{Digest, Sha256};

const TAG: &str = "ssh-rsa";

pub struct SshRsaIdentity {
    private_key: RsaPrivateKey,
}

impl SshRsaIdentity {
    pub fn from_private_key(private_key: RsaPrivateKey) -> Self {
        Self { private_key }
    }

    fn fingerprint(&self) -> [u8; 4] {
        rsa_pubkey_fingerprint(&RsaPublicKey::from(&self.private_key))
    }
}

impl Identity for SshRsaIdentity {
    fn unwrap_file_key(&self, stanza: &Stanza) -> Option<Result<FileKey, AgeError>> {
        if stanza.tag != TAG {
            return None;
        }
        if stanza.args.len() != 1 {
            return Some(Err(AgeError::ParseError(
                "ssh-rsa stanza must have 1 arg".to_string(),
            )));
        }
        let fp = match STANDARD_NO_PAD.decode(&stanza.args[0]) {
            Ok(b) => b,
            Err(e) => {
                return Some(Err(AgeError::ParseError(format!(
                    "fingerprint base64: {e}"
                ))));
            }
        };
        if fp.as_slice() != self.fingerprint() {
            return None;
        }
        Some(unwrap(stanza, &self.private_key))
    }
}

fn unwrap(stanza: &Stanza, private_key: &RsaPrivateKey) -> Result<FileKey, AgeError> {
    let padding = Oaep::new::<Sha256>();
    let file_key_bytes = private_key
        .decrypt(padding, &stanza.body)
        .map_err(|_| AgeError::CryptoError("RSA-OAEP decrypt failed".to_string()))?;
    FileKey::try_from(file_key_bytes.as_slice())
}

pub(crate) fn rsa_pubkey_fingerprint(public_key: &RsaPublicKey) -> [u8; 4] {
    let n_bytes = public_key.n().to_bytes_be();
    let e_bytes = public_key.e().to_bytes_be();
    let mut hasher = Sha256::new();
    hasher.update(&n_bytes);
    hasher.update(&e_bytes);
    let hash = hasher.finalize();
    hash[..4].try_into().expect("SHA-256 is 32 bytes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipients::Recipient;
    use crate::recipients::ssh_rsa::SshRsaRecipient;
    use rand::rngs::OsRng;

    fn test_rsa_keypair() -> (RsaPrivateKey, RsaPublicKey) {
        let private_key = RsaPrivateKey::new(&mut OsRng, 2048).expect("RSA keygen failed");
        let public_key = RsaPublicKey::from(&private_key);
        (private_key, public_key)
    }

    #[test]
    fn ssh_rsa_wrap_unwrap_roundtrip() {
        let (private_key, public_key) = test_rsa_keypair();
        let recipient = SshRsaRecipient::from_public_key(public_key);
        let identity = SshRsaIdentity::from_private_key(private_key);

        let file_key = FileKey::new([0x55u8; 16]);
        let stanza = recipient
            .wrap_file_key(&file_key)
            .expect("wrap should succeed");
        assert_eq!(stanza.tag, "ssh-rsa");
        assert_eq!(stanza.args.len(), 1);

        let unwrapped = identity
            .unwrap_file_key(&stanza)
            .expect("identity should match")
            .expect("unwrap should succeed");
        assert_eq!(unwrapped.as_bytes(), file_key.as_bytes());
    }

    #[test]
    fn ssh_rsa_wrong_fingerprint_returns_none() {
        let (private_key1, _) = test_rsa_keypair();
        let (_, public_key2) = test_rsa_keypair();
        let recipient = SshRsaRecipient::from_public_key(public_key2);
        let identity = SshRsaIdentity::from_private_key(private_key1);

        let file_key = FileKey::new([0x55u8; 16]);
        let stanza = recipient.wrap_file_key(&file_key).unwrap();
        assert!(identity.unwrap_file_key(&stanza).is_none());
    }
}
