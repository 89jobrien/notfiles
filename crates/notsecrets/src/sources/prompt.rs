use anyhow::Result;
use crate::AgeKeySource;

pub struct PromptSource;

impl AgeKeySource for PromptSource {
    fn name(&self) -> &str { "prompt" }

    fn retrieve(&self) -> Result<String> {
        let key = rpassword::prompt_password("Paste your age private key: ")
            .map_err(|e| anyhow::anyhow!("could not read age key from prompt: {e}"))?;
        if key.trim().is_empty() {
            anyhow::bail!("empty age key entered");
        }
        Ok(key)
    }
}
