use std::path::{Path, PathBuf};

use serde_json::json;

use crate::linker::State;
use crate::ports::FileStore;
use notcore::{Method, expand_tilde};

#[derive(Debug, Clone, PartialEq)]
pub struct WhichResult {
    pub package: String,
    pub source: PathBuf,
    pub target: PathBuf,
    pub method: Option<Method>,
    pub linked_at: Option<String>,
    pub managed: bool,
}

/// Resolve which package/source a managed target path came from.
///
/// Checks `.notfiles-state.toml` first. If no entry matches, falls back to
/// resolving the path as a symlink and checking whether it points somewhere
/// under `dotfiles_dir` — reported as unmanaged (not tracked in state).
pub fn locate(
    path: &Path,
    dotfiles_dir: &Path,
    state: &State,
    fs: &dyn FileStore,
) -> Option<WhichResult> {
    let target = resolve_input(path);
    let target_str = target.to_string_lossy();

    if let Some(entry) = state.entries.iter().find(|e| e.target == target_str) {
        return Some(WhichResult {
            package: entry.package.clone(),
            source: PathBuf::from(&entry.source),
            target,
            method: Some(entry.method),
            linked_at: Some(entry.linked_at.clone()),
            managed: true,
        });
    }

    let source = fs.read_link(&target).ok()?;
    if !source.starts_with(dotfiles_dir) {
        return None;
    }
    let package = source
        .strip_prefix(dotfiles_dir)
        .ok()?
        .components()
        .next()?
        .as_os_str()
        .to_string_lossy()
        .to_string();

    Some(WhichResult {
        package,
        source,
        target,
        method: None,
        linked_at: None,
        managed: false,
    })
}

fn resolve_input(path: &Path) -> PathBuf {
    match expand_tilde(&path.to_string_lossy()) {
        Ok(expanded) => expanded,
        Err(_) => path.to_path_buf(),
    }
}

pub fn print_which(result: &WhichResult) {
    let managed_label = if result.managed {
        "\x1b[32mmanaged\x1b[0m"
    } else {
        "\x1b[33munmanaged (not in state)\x1b[0m"
    };
    println!(
        "{managed_label} {} -> {} (package: {})",
        result.target.display(),
        result.source.display(),
        result.package
    );
    if let Some(method) = result.method {
        println!("  method: {method}");
    }
    if let Some(linked_at) = &result.linked_at {
        println!("  linked_at: {linked_at}");
    }
}

pub fn print_which_json(result: &WhichResult) {
    let obj = json!({
        "package": result.package,
        "source": result.source.to_string_lossy(),
        "target": result.target.to_string_lossy(),
        "method": result.method.map(|m| m.to_string()),
        "linked_at": result.linked_at,
        "managed": result.managed,
    });
    println!("{}", serde_json::to_string(&obj).unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::memory::InMemoryFileStore;
    use crate::linker::StateEntry;

    fn state_with_entry() -> State {
        State {
            entries: vec![StateEntry {
                package: "git".to_string(),
                source: "/dotfiles/git/.gitconfig".to_string(),
                target: "/home/.gitconfig".to_string(),
                method: Method::Symlink,
                linked_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        }
    }

    #[test]
    fn locate_matches_tracked_state_entry() {
        let fs = InMemoryFileStore::new();
        let state = state_with_entry();

        let result = locate(
            Path::new("/home/.gitconfig"),
            Path::new("/dotfiles"),
            &state,
            &fs,
        );

        assert_eq!(
            result,
            Some(WhichResult {
                package: "git".to_string(),
                source: PathBuf::from("/dotfiles/git/.gitconfig"),
                target: PathBuf::from("/home/.gitconfig"),
                method: Some(Method::Symlink),
                linked_at: Some("2026-01-01T00:00:00Z".to_string()),
                managed: true,
            })
        );
    }

    #[test]
    fn locate_falls_back_to_read_link_when_untracked() {
        let fs = InMemoryFileStore::new();
        fs.add_file("/dotfiles/vim/.vimrc", b"content");
        fs.symlink(Path::new("/dotfiles/vim/.vimrc"), Path::new("/home/.vimrc"))
            .unwrap();
        let state = State::default();

        let result = locate(
            Path::new("/home/.vimrc"),
            Path::new("/dotfiles"),
            &state,
            &fs,
        );

        assert_eq!(
            result,
            Some(WhichResult {
                package: "vim".to_string(),
                source: PathBuf::from("/dotfiles/vim/.vimrc"),
                target: PathBuf::from("/home/.vimrc"),
                method: None,
                linked_at: None,
                managed: false,
            })
        );
    }

    #[test]
    fn locate_returns_none_when_not_found() {
        let fs = InMemoryFileStore::new();
        let state = State::default();

        let result = locate(
            Path::new("/home/.unknown"),
            Path::new("/dotfiles"),
            &state,
            &fs,
        );

        assert_eq!(result, None);
    }
}
