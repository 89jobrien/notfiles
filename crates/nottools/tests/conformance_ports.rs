//! Contracts for the `Fs` and `ToolProbe` ports, run against the system adapter and its double.
//!
//! `Installer` has no suite: `SystemInstaller` runs `mise` / `brew install` for real, and the
//! `RecordingInstaller` spy is exercised through the `apply_fixes` tests.

use nottools::adapters::{FakeProbe, MemoryFs, SystemFs, SystemProbe};
use nottools::{Fs, ToolProbe};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn assert_fs_contract(fs: &dyn Fs, root: &Path) {
    let a = root.join("a.txt");
    fs.write(&a, "one").expect("write");
    assert_eq!(fs.read_to_string(&a).expect("read"), "one");

    fs.write(&a, "two").expect("overwrite");
    assert_eq!(fs.read_to_string(&a).expect("read after overwrite"), "two");

    let err = fs
        .read_to_string(&root.join("missing"))
        .expect_err("missing file");
    assert_eq!(err.kind(), ErrorKind::NotFound);

    fs.write(&root.join("b.txt"), "b").expect("write b");
    let listed = fs.list_dir(root).expect("list_dir");
    assert_eq!(
        listed,
        vec![root.join("a.txt"), root.join("b.txt")],
        "direct children, sorted"
    );

    assert!(
        fs.list_dir(&root.join("no-such-dir")).is_err(),
        "missing dir is an error"
    );
}

#[test]
fn system_fs_satisfies_contract() {
    let dir = TempDir::new().expect("tempdir");
    assert_fs_contract(&SystemFs, dir.path());
}

#[test]
fn memory_fs_satisfies_contract() {
    assert_fs_contract(&MemoryFs::default(), Path::new("/root"));
}

/// `present_*` must resolve and `absent_*` must not.
fn assert_probe_contract(
    probe: &dyn ToolProbe,
    present_bin: &str,
    absent_bin: &str,
    present_path: &Path,
    absent_path: &Path,
) {
    assert!(probe.on_path(present_bin));
    assert!(!probe.on_path(absent_bin));
    assert!(probe.path_exists(present_path));
    assert!(!probe.path_exists(absent_path));
    // Whatever is installed, listing it must not panic and names must be non-empty.
    assert!(probe.installed_crates().iter().all(|c| !c.name.is_empty()));
}

#[test]
fn system_probe_satisfies_contract() {
    let dir = TempDir::new().expect("tempdir");
    assert_probe_contract(
        &SystemProbe,
        "sh",
        "nottools-no-such-binary",
        dir.path(),
        &dir.path().join("missing"),
    );
}

#[test]
fn fake_probe_satisfies_contract() {
    let probe = FakeProbe::new(&["sh"], &["/present"], Vec::new());
    assert_probe_contract(
        &probe,
        "sh",
        "nottools-no-such-binary",
        &PathBuf::from("/present"),
        &PathBuf::from("/absent"),
    );
}
