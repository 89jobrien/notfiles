use std::fs;
use tempfile::TempDir;
use notsecrets::{resolve_age_key, AgeKeySource, FileSource};

#[test]
fn test_file_source_reads_key() {
    let dir = TempDir::new().unwrap();
    let key_file = dir.path().join("age.key");
    fs::write(&key_file, "AGE-SECRET-KEY-1ABCDEF\n").unwrap();

    let source = FileSource::new(key_file.clone());
    let result = source.retrieve();
    assert!(result.is_ok());
    assert_eq!(result.unwrap().trim(), "AGE-SECRET-KEY-1ABCDEF");
}

#[test]
fn test_file_source_missing_returns_err() {
    let source = FileSource::new("/nonexistent/age.key".into());
    assert!(source.retrieve().is_err());
}

#[test]
fn test_resolve_age_key_uses_file_fallback() {
    let dir = TempDir::new().unwrap();
    let key_file = dir.path().join("age.key");
    fs::write(&key_file, "AGE-SECRET-KEY-1TEST\n").unwrap();

    let sources: Vec<Box<dyn AgeKeySource>> = vec![
        Box::new(FileSource::new(key_file)),
    ];
    let key = resolve_age_key(sources).unwrap();
    assert_eq!(key.trim(), "AGE-SECRET-KEY-1TEST");
}

#[test]
fn test_resolve_age_key_all_fail_returns_err() {
    let sources: Vec<Box<dyn AgeKeySource>> = vec![
        Box::new(FileSource::new("/nonexistent".into())),
    ];
    assert!(resolve_age_key(sources).is_err());
}
