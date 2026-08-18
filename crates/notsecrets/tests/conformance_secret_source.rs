use notsecrets::ports::{EnumerableSecretSource, SecretSource};
use notsecrets::sources::*;

fn assert_secret_source_contract(source: &dyn SecretSource) {
    assert!(
        !source.name().is_empty(),
        "{:?}: name() must be non-empty",
        source.provider()
    );

    // resolve for unknown key returns Ok(None)
    let result = source.resolve("__CONFORMANCE_NONEXISTENT_KEY_99999__");
    assert!(
        result.is_ok(),
        "{}: resolve(unknown) should be Ok, got {:?}",
        source.name(),
        result,
    );
    assert_eq!(
        result.unwrap(),
        None,
        "{}: resolve(unknown) should be None",
        source.name(),
    );
}

fn assert_enumerable_contract(source: &dyn EnumerableSecretSource) {
    assert_secret_source_contract(source);

    let map = source.resolve_all();
    assert!(
        map.is_ok(),
        "{}: resolve_all() should be Ok, got {:?}",
        source.name(),
        map,
    );
}

#[test]
fn conformance_env_source() {
    assert_enumerable_contract(&EnvSource);
}

#[test]
fn conformance_op_source() {
    assert_secret_source_contract(&OpSource::new("test.1password.com".to_string()));
}

#[test]
fn conformance_gsm_source() {
    assert_secret_source_contract(&GsmSource::new("test-project".to_string()));
}

#[test]
fn conformance_nuenv_stub() {
    assert_secret_source_contract(&NuenvSource);
}

#[test]
fn conformance_direnv_stub() {
    assert_secret_source_contract(&DirenvSource);
}

#[test]
fn conformance_mise_stub() {
    assert_secret_source_contract(&MiseSource);
}

#[test]
fn conformance_vault_stub() {
    assert_secret_source_contract(&VaultSource);
}

#[test]
fn conformance_dotenvy_stub() {
    assert_secret_source_contract(&DotenvySource);
}

#[test]
fn conformance_bitwarden_source() {
    // Bitwarden resolves only via an explicit binding, so the bare-key contract
    // holds without ever invoking the `bw` CLI.
    assert_secret_source_contract(&BitwardenSource::new("test-item"));
}
