use std::path::PathBuf;
use anyhow::Result;
use crate::AgeKeySource;

pub struct FileSource {
    path: PathBuf,
}

impl FileSource {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl AgeKeySource for FileSource {
    fn name(&self) -> &str { "file" }

    fn retrieve(&self) -> Result<String> {
        std::fs::read_to_string(&self.path)
            .map_err(|e| anyhow::anyhow!("cannot read key file {}: {e}", self.path.display()))
    }
}
