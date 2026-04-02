#[derive(Debug, thiserror::Error)]
pub enum AgeError {
    #[error("no identity could decrypt any recipient stanza")]
    NoMatch,
    #[error("header MAC verification failed")]
    MacMismatch,
    #[error("malformed age file: {0}")]
    ParseError(String),
    #[error("crypto error: {0}")]
    CryptoError(String),
    #[error("unsupported key type: {0}")]
    UnsupportedKeyType(String),
    #[error("identity source failed ({name}): {source}")]
    SourceError { name: String, source: anyhow::Error },
}
