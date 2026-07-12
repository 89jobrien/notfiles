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

#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("[{name}] {source}")]
    SourceError { name: String, source: anyhow::Error },
    #[error("no provider resolved key: {key}")]
    NotFound { key: String },
    #[error("config error: {0}")]
    Config(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_error_display_source_error() {
        let err = SecretsError::SourceError {
            name: "op".to_string(),
            source: anyhow::anyhow!("not found"),
        };
        assert!(err.to_string().contains("[op]"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn secrets_error_display_not_found() {
        let err = SecretsError::NotFound {
            key: "DB_URL".to_string(),
        };
        assert!(err.to_string().contains("DB_URL"));
    }

    #[test]
    fn secrets_error_display_config() {
        let err = SecretsError::Config("bad toml".to_string());
        assert!(err.to_string().contains("bad toml"));
    }
}
