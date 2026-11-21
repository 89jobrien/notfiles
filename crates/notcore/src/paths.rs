use std::path::PathBuf;

use crate::NotfilesError;

pub fn expand_tilde(path: &str) -> Result<PathBuf, NotfilesError> {
    if path == "~" {
        return dirs::home_dir()
            .ok_or_else(|| NotfilesError::Path("cannot determine home directory".into()));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        let home = dirs::home_dir()
            .ok_or_else(|| NotfilesError::Path("cannot determine home directory".into()))?;
        return Ok(home.join(rest));
    }
    Ok(PathBuf::from(path))
}

pub fn dotfiles_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join("dotfiles"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde_home() {
        let home = dirs::home_dir().expect("home_dir should succeed");
        assert_eq!(
            expand_tilde("~").expect("expand_tilde should succeed"),
            home
        );
    }

    #[test]
    fn test_expand_tilde_subpath() {
        let home = dirs::home_dir().expect("home_dir should succeed");
        assert_eq!(
            expand_tilde("~/foo/bar").expect("expand_tilde should succeed"),
            home.join("foo/bar")
        );
    }

    #[test]
    fn test_expand_tilde_absolute() {
        assert_eq!(
            expand_tilde("/usr/bin").expect("expand_tilde should succeed"),
            PathBuf::from("/usr/bin")
        );
    }
}
