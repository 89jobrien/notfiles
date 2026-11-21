//! One contract for every `FileStore`. The real filesystem defines the behaviour; the
//! in-memory double must match it for everything the linker relies on.
//!
//! Not covered: `metadata` / `symlink_metadata` (they return `std::fs::Metadata`, which an
//! in-memory store cannot construct) and `copy_permissions` (the double models no modes).

use notfiles::FileStore;
use notfiles::adapters::{FileStoreImpl, InMemoryFileStore};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn kind<T>(result: Result<T, std::io::Error>) -> ErrorKind {
    result.err().expect("expected an error").kind()
}

fn sorted(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort();
    paths
}

fn assert_file_store_contract(store: &dyn FileStore, root: &Path) {
    // -- files ---------------------------------------------------------------------------
    store
        .create_dir_all(&root.join("a/b"))
        .expect("create_dir_all");
    store
        .create_dir_all(&root.join("a/b"))
        .expect("create_dir_all is idempotent");
    assert!(store.is_dir(&root.join("a")) && store.is_dir(&root.join("a/b")));

    let file = root.join("a/f.txt");
    store.write(&file, b"hello").expect("write");
    assert_eq!(store.read(&file).expect("read"), b"hello");
    assert_eq!(
        store.read_to_string(&file).expect("read_to_string"),
        "hello"
    );
    store.write(&file, b"bye").expect("overwrite");
    assert_eq!(store.read(&file).expect("read after overwrite"), b"bye");

    assert!(store.exists(&file) && !store.is_dir(&file));
    assert!(!store.exists(&root.join("a/missing")));
    assert_eq!(
        kind(store.read(&root.join("a/missing"))),
        ErrorKind::NotFound
    );

    // -- read_dir ------------------------------------------------------------------------
    assert_eq!(
        sorted(store.read_dir(&root.join("a")).expect("read_dir")),
        vec![root.join("a/b"), root.join("a/f.txt")],
        "direct children only"
    );
    assert!(
        store.read_dir(&root.join("nope")).is_err(),
        "missing dir is an error"
    );

    // -- rename --------------------------------------------------------------------------
    let moved = root.join("a/g.txt");
    store.rename(&file, &moved).expect("rename");
    assert!(!store.exists(&file));
    assert_eq!(store.read(&moved).expect("read renamed"), b"bye");
    assert_eq!(kind(store.rename(&file, &moved)), ErrorKind::NotFound);

    // -- symlinks ------------------------------------------------------------------------
    #[cfg(unix)]
    {
        let link = root.join("a/link");
        store.symlink(&moved, &link).expect("symlink");
        assert_eq!(store.read_link(&link).expect("read_link"), moved);
        assert_eq!(store.read(&link).expect("read through link"), b"bye");
        assert!(store.exists(&link));
        assert_eq!(
            kind(store.symlink(&moved, &link)),
            ErrorKind::AlreadyExists,
            "symlink never overwrites"
        );

        store
            .remove_file(&link)
            .expect("remove_file removes the link");
        assert!(!store.exists(&link));
        assert!(store.exists(&moved), "the target is untouched");
    }

    // -- removal -------------------------------------------------------------------------
    store.remove_file(&moved).expect("remove_file");
    assert!(!store.exists(&moved));
    assert_eq!(kind(store.remove_file(&moved)), ErrorKind::NotFound);

    store.create_dir_all(&root.join("d/inner")).expect("mkdir");
    store.write(&root.join("d/inner/x"), b"x").expect("write x");
    assert!(
        store.remove_dir(&root.join("d")).is_err(),
        "remove_dir refuses a non-empty dir"
    );
    store
        .remove_dir_all(&root.join("d"))
        .expect("remove_dir_all");
    assert!(!store.exists(&root.join("d")) && !store.exists(&root.join("d/inner/x")));
    assert_eq!(
        kind(store.remove_dir_all(&root.join("d"))),
        ErrorKind::NotFound
    );

    store
        .create_dir_all(&root.join("empty"))
        .expect("mkdir empty");
    store
        .remove_dir(&root.join("empty"))
        .expect("remove_dir on an empty dir");
    assert!(!store.exists(&root.join("empty")));
    assert_eq!(
        kind(store.remove_dir(&root.join("empty"))),
        ErrorKind::NotFound
    );
}

#[test]
fn real_file_store_satisfies_contract() {
    let dir = TempDir::new().expect("tempdir");
    assert_file_store_contract(&FileStoreImpl, dir.path());
}

#[test]
fn in_memory_file_store_satisfies_contract() {
    assert_file_store_contract(&InMemoryFileStore::new(), Path::new("/root"));
}
