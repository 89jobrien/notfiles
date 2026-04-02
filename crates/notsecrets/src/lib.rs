pub mod error;
pub mod format;
pub mod identities;
pub mod recipients;
pub mod sources;
pub mod _legacy;

pub use error::AgeError;
pub use identities::{FileKey, Header, Identity, Stanza, X25519Identity, ScryptIdentity};
pub use recipients::{Recipient, X25519Recipient, ScryptRecipient};
pub use sources::{BitwardenSource, FileSource, PromptSource};

// Legacy API — kept for notstrap compatibility until Task 12.
#[allow(deprecated)]
pub use _legacy::{
    AgeKeySource,
    install_age_key,
    install_age_key_at,
    resolve_age_key,
    decrypt_sops,
    sops_args,
};
