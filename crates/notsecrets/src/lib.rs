pub mod decrypt;
pub mod encrypt;
pub mod error;
pub mod format;
pub mod identities;
pub mod ports;
pub mod recipients;
pub mod sources;

pub use decrypt::Decryptor;
pub use encrypt::Encryptor;
pub use error::AgeError;
pub use identities::{
    EncryptedIdentity, FileKey, Header, Identity, ScryptIdentity, SshEd25519Identity,
    SshRsaIdentity, Stanza, X25519Identity,
};
pub use ports::IdentitySource;
pub use recipients::{
    Recipient, ScryptRecipient, SshEd25519Recipient, SshRsaRecipient, X25519Recipient,
};
pub use sources::{BitwardenSource, FileSource, PromptSource};

/// Try each `IdentitySource` in order; collect all identities that load successfully.
///
/// Returns an error only if all sources fail. Partial success is accepted because
/// not every source needs to hold the key for the target file.
pub fn resolve_identities(
    sources: Vec<Box<dyn IdentitySource>>,
) -> Result<Vec<Box<dyn Identity>>, AgeError> {
    let mut identities: Vec<Box<dyn Identity>> = Vec::new();
    let mut last_err: Option<AgeError> = None;

    for source in sources {
        match source.load() {
            Ok(identity) => identities.push(identity),
            Err(e) => {
                eprintln!("  [{}] {e}", source.name());
                last_err = Some(e);
            }
        }
    }

    if identities.is_empty() {
        Err(last_err.unwrap_or(AgeError::SourceError {
            name: "resolve_identities".to_string(),
            source: anyhow::anyhow!("no sources provided"),
        }))
    } else {
        Ok(identities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::IdentitySource;

    struct AlwaysFailSource;
    impl IdentitySource for AlwaysFailSource {
        fn name(&self) -> &str {
            "always-fail"
        }
        fn load(&self) -> Result<Box<dyn Identity>, AgeError> {
            Err(AgeError::SourceError {
                name: "always-fail".to_string(),
                source: anyhow::anyhow!("intentional failure"),
            })
        }
    }

    struct StaticX25519Source;
    impl IdentitySource for StaticX25519Source {
        fn name(&self) -> &str {
            "static-x25519"
        }
        fn load(&self) -> Result<Box<dyn Identity>, AgeError> {
            use rand::rngs::OsRng;
            use x25519_dalek::StaticSecret;
            Ok(Box::new(
                crate::identities::x25519::X25519Identity::from_static_secret(
                    StaticSecret::random_from_rng(OsRng),
                ),
            ))
        }
    }

    #[test]
    fn resolve_identities_all_fail_returns_error() {
        let sources: Vec<Box<dyn IdentitySource>> = vec![Box::new(AlwaysFailSource)];
        assert!(resolve_identities(sources).is_err());
    }

    #[test]
    fn resolve_identities_partial_success_returns_loaded() {
        let sources: Vec<Box<dyn IdentitySource>> =
            vec![Box::new(AlwaysFailSource), Box::new(StaticX25519Source)];
        let ids = resolve_identities(sources).expect("at least one source succeeded");
        assert_eq!(ids.len(), 1);
    }
}
