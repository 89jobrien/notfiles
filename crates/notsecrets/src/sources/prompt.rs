use crate::error::AgeError;
use crate::identities::Identity;
use crate::identities::scrypt::ScryptIdentity;
use crate::ports::IdentitySource;

pub struct PromptSource;

impl IdentitySource for PromptSource {
    fn name(&self) -> &str {
        "prompt"
    }

    fn load(&self) -> Result<Box<dyn Identity>, AgeError> {
        let passphrase = rpassword::prompt_password("Enter age passphrase: ").map_err(|e| {
            AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("could not read passphrase: {e}"),
            }
        })?;
        if passphrase.trim().is_empty() {
            return Err(AgeError::SourceError {
                name: self.name().to_string(),
                source: anyhow::anyhow!("empty passphrase entered"),
            });
        }
        Ok(Box::new(ScryptIdentity::new(passphrase)))
    }
}
