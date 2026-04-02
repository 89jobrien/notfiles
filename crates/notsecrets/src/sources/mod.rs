use crate::error::AgeError;
use crate::identities::Identity;

pub mod bitwarden;
pub mod file;
pub mod prompt;

pub use bitwarden::BitwardenSource;
pub use file::FileSource;
pub use prompt::PromptSource;

/// Infra boundary trait: an identity resolver that loads key material from an
/// external source and returns a concrete Identity.
pub trait IdentitySource {
    fn name(&self) -> &str;
    fn load(&self) -> Result<Box<dyn Identity>, AgeError>;
}
