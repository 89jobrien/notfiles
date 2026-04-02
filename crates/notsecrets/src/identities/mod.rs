use crate::error::AgeError;

pub mod x25519;
pub use x25519::X25519Identity;

/// The symmetric file encryption key — 16 random bytes.
#[derive(Clone)]
pub struct FileKey([u8; 16]);

impl FileKey {
    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn generate() -> Self {
        use rand::RngCore;
        let mut bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(bytes)
    }
}

impl TryFrom<&[u8]> for FileKey {
    type Error = AgeError;
    fn try_from(b: &[u8]) -> Result<Self, AgeError> {
        b.try_into()
            .map(Self)
            .map_err(|_| AgeError::ParseError(format!("file key must be 16 bytes, got {}", b.len())))
    }
}

impl Drop for FileKey {
    fn drop(&mut self) {
        // Zero out key material on drop
        self.0.iter_mut().for_each(|b| *b = 0);
    }
}

/// A single recipient stanza in the age header.
#[derive(Debug, Clone)]
pub struct Stanza {
    pub tag: String,
    pub args: Vec<String>,
    pub body: Vec<u8>,
}

/// The full age header (all stanzas + MAC).
#[derive(Debug, Clone)]
pub struct Header {
    pub recipients: Vec<Stanza>,
    pub mac: Vec<u8>,
}

/// Domain port: an identity that can attempt to unwrap a recipient stanza.
///
/// Returns `None` if the stanza tag/args do not match this identity type.
/// Returns `Some(Err(...))` if the stanza matches but decryption fails.
/// Returns `Some(Ok(file_key))` on success.
pub trait Identity {
    fn unwrap_file_key(&self, stanza: &Stanza) -> Option<Result<FileKey, AgeError>>;
}
