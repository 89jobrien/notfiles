use crate::error::AgeError;
use crate::identities::{FileKey, Stanza};

/// Domain port: a recipient that can wrap a file key into a stanza.
pub trait Recipient {
    fn wrap_file_key(&self, file_key: &FileKey) -> Result<Stanza, AgeError>;
}
