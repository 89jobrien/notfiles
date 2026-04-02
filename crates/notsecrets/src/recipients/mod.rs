use crate::error::AgeError;
use crate::identities::{FileKey, Stanza};

pub mod x25519;
pub use x25519::X25519Recipient;

pub mod ssh_ed25519;
pub use ssh_ed25519::SshEd25519Recipient;

pub mod scrypt;
pub use scrypt::ScryptRecipient;

pub mod ssh_rsa;
pub use ssh_rsa::SshRsaRecipient;

/// Domain port: a recipient that can wrap a file key into a stanza.
pub trait Recipient {
    fn wrap_file_key(&self, file_key: &FileKey) -> Result<Stanza, AgeError>;
}
