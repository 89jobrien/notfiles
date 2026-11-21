//! `detect` scenarios, each run against the real filesystem and the in-memory store.

use notfiles::FileStore;
use notfiles::adapters::{FileStoreImpl, InMemoryFileStore};
use notfiles::detect::{DetectedManager, ManagerKind, detect};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn put(store: &dyn FileStore, path: &Path, content: &str) {
    store
        .create_dir_all(path.parent().expect("file has a parent"))
        .expect("create parents");
    store.write(path, content.as_bytes()).expect("write file");
}

fn names(m: &DetectedManager) -> Vec<&str> {
    m.packages.iter().map(|p| p.name.as_str()).collect()
}

fn files<'a>(m: &'a DetectedManager, pkg: &str) -> &'a [PathBuf] {
    &m.packages
        .iter()
        .find(|p| p.name == pkg)
        .expect("package present")
        .files
}

/// Run `scenario` against both stores, each with its own home directory.
fn on_both(scenario: impl Fn(&dyn FileStore, &Path)) {
    let dir = TempDir::new().expect("tempdir");
    scenario(&FileStoreImpl, dir.path());
    scenario(&InMemoryFileStore::new(), Path::new("/home/me"));
}

#[test]
fn empty_home_detects_nothing() {
    on_both(|store, home| {
        store.create_dir_all(home).expect("mkdir home");
        assert!(detect(store, home).is_empty());
    });
}

#[test]
fn notfiles_toml_marks_a_notfiles_repo_and_lists_included_packages() {
    on_both(|store, home| {
        let root = home.join("dotfiles");
        put(
            store,
            &root.join("notfiles.toml"),
            "include = [\"zsh\", 'git']\n",
        );
        put(store, &root.join("zsh/.zshrc"), "# z");
        put(store, &root.join("git/.config/git/config"), "# g");
        put(store, &root.join("ignored/.x"), "# not included");

        let found = detect(store, home);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, ManagerKind::Notfiles);
        assert_eq!(found[0].root, root);
        assert_eq!(names(&found[0]), ["zsh", "git"]);
        assert_eq!(
            files(&found[0], "git"),
            [PathBuf::from(".config/git/config")]
        );
    });
}

#[test]
fn notfiles_toml_without_include_falls_back_to_directory_packages() {
    on_both(|store, home| {
        let root = home.join(".dotfiles");
        put(store, &root.join("notfiles.toml"), "[defaults]\n");
        put(store, &root.join("zsh/.zshrc"), "# z");

        let found = detect(store, home);

        assert_eq!(found[0].kind, ManagerKind::Notfiles);
        assert_eq!(names(&found[0]), ["zsh"]);
    });
}

#[test]
fn stow_marker_file_identifies_stow_and_skips_housekeeping_dirs() {
    on_both(|store, home| {
        let root = home.join("dots");
        put(store, &root.join(".stowrc"), "");
        put(store, &root.join("vim/.vimrc"), "");
        put(store, &root.join("vim/.git/HEAD"), "ref");
        put(store, &root.join("vim/.DS_Store"), "junk");
        put(store, &root.join("docs/README.md"), "");
        put(store, &root.join("target/x"), "");

        let found = detect(store, home);

        assert_eq!(found[0].kind, ManagerKind::Stow);
        assert_eq!(names(&found[0]), ["vim"]);
        assert_eq!(files(&found[0], "vim"), [PathBuf::from(".vimrc")]);
    });
}

#[test]
fn two_packages_with_dot_content_look_like_stow_but_one_does_not() {
    on_both(|store, home| {
        let two = home.join("dotfiles");
        put(store, &two.join("zsh/.zshrc"), "");
        put(store, &two.join("git/.gitconfig"), "");
        let one = home.join(".dots");
        put(store, &one.join("zsh/.zshrc"), "");
        put(store, &one.join("notes/readme.txt"), "");

        let found = detect(store, home);
        let kind_of = |root: &Path| {
            found
                .iter()
                .find(|m| m.root == root)
                .expect("root detected")
                .kind
                .clone()
        };

        assert_eq!(kind_of(&two), ManagerKind::Stow);
        assert_eq!(kind_of(&one), ManagerKind::Unknown);
    });
}

#[test]
fn chezmoi_marker_yields_a_single_source_package() {
    on_both(|store, home| {
        let root = home.join("dotfiles");
        put(store, &root.join(".chezmoi.toml"), "");
        put(store, &root.join("dot_zshrc"), "");
        put(store, &root.join("private_dot_ssh/config"), "");

        let found = detect(store, home);

        assert_eq!(found[0].kind, ManagerKind::Chezmoi);
        assert_eq!(names(&found[0]), ["(source)"]);
        assert_eq!(files(&found[0], "(source)").len(), 3);
    });
}

#[test]
fn chezmoi_default_source_dir_is_found_once() {
    on_both(|store, home| {
        let src = home.join(".local/share/chezmoi");
        put(store, &src.join("dot_zshrc"), "");

        let found = detect(store, home);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, ManagerKind::Chezmoi);
        assert_eq!(found[0].root, src);
    });
}
