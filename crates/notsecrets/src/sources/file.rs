use crate::error::AgeError;
use crate::identities::Identity;
use crate::identities::encrypted::EncryptedIdentity;
use crate::identities::x25519::X25519Identity;
use crate::ports::IdentitySource;
use std::path::PathBuf;

pub struct FileSource {
    path: PathBuf,
}

impl FileSource {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl IdentitySource for FileSource {
    fn name(&self) -> &str {
        "file"
    }

    fn load(&self) -> Result<Box<dyn Identity>, AgeError> {
        let content = std::fs::read(&self.path).map_err(|e| AgeError::SourceError {
            name: self.name().to_string(),
            source: anyhow::anyhow!("cannot read key file {}: {e}", self.path.display()),
        })?;

        if content.starts_with(b"AGE-SECRET-KEY-1") {
            let key_str = std::str::from_utf8(&content)
                .map_err(|e| AgeError::SourceError {
                    name: self.name().to_string(),
                    source: anyhow::anyhow!("key file UTF-8: {e}"),
                })?
                .trim();
            let identity =
                X25519Identity::from_bech32(key_str).map_err(|e| AgeError::SourceError {
                    name: self.name().to_string(),
                    source: anyhow::anyhow!("invalid AGE-SECRET-KEY-1: {e}"),
                })?;
            Ok(Box::new(identity))
        } else if content.starts_with(b"age-encryption.org/v1") {
            let passphrase =
                rpassword::prompt_password("Enter passphrase for encrypted identity file: ")
                    .map_err(|e| AgeError::SourceError {
                        name: self.name().to_string(),
                        source: anyhow::anyhow!("could not read passphrase: {e}"),
                    })?;
            Ok(Box::new(EncryptedIdentity::new(content, passphrase)))
        } else {
            Err(AgeError::UnsupportedKeyType(format!(
                "file {} does not contain a recognized age key format",
                self.path.display()
            )))
        }
    }
}
