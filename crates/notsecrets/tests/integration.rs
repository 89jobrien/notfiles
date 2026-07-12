use notsecrets::{FileSource, IdentitySource, X25519Identity, resolve_identities};
use std::fs;
use tempfile::TempDir;

fn generate_age_key_str() -> String {
    use rand::rngs::OsRng;
    use x25519_dalek::StaticSecret;
    let identity = X25519Identity::from_static_secret(StaticSecret::random_from_rng(OsRng));
    identity.to_bech32()
}

#[test]
fn test_file_source_loads_valid_key() {
    let dir = TempDir::new().unwrap();
    let key_file = dir.path().join("age.key");
    let key = generate_age_key_str();
    fs::write(&key_file, format!("{}\n", key)).unwrap();

    let source = FileSource::new(key_file.clone());
    let result = source.load();
    assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
}

#[test]
fn test_file_source_missing_returns_err() {
    let source = FileSource::new("/nonexistent/age.key".into());
    assert!(source.load().is_err());
}

#[test]
fn test_resolve_identities_uses_file_source() {
    let dir = TempDir::new().unwrap();
    let key_file = dir.path().join("age.key");
    let key = generate_age_key_str();
    fs::write(&key_file, format!("{}\n", key)).unwrap();

    let sources: Vec<Box<dyn IdentitySource>> = vec![Box::new(FileSource::new(key_file))];
    let ids = resolve_identities(sources).unwrap();
    assert_eq!(ids.len(), 1);
}

#[test]
fn test_resolve_identities_all_fail_returns_err() {
    let sources: Vec<Box<dyn IdentitySource>> =
        vec![Box::new(FileSource::new("/nonexistent".into()))];
    assert!(resolve_identities(sources).is_err());
}
