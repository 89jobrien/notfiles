use crate::error::AgeError;
use crate::identities::Identity;

/// Infra boundary trait: an identity resolver that loads key material from an
/// external source and returns a concrete Identity.
pub trait IdentitySource {
    /// Human-readable name of the identity source (e.g., "bitwarden", "file", "prompt").
    fn name(&self) -> &str;

    /// Load and return a concrete identity from the source.
    fn load(&self) -> Result<Box<dyn Identity>, AgeError>;
}
