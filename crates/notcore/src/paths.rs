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
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_tilde("~").unwrap(), home);
    }

    #[test]
    fn test_expand_tilde_subpath() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_tilde("~/foo/bar").unwrap(), home.join("foo/bar"));
    }

    #[test]
    fn test_expand_tilde_absolute() {
        assert_eq!(expand_tilde("/usr/bin").unwrap(), PathBuf::from("/usr/bin"));
    }
}
