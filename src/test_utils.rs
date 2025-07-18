use std::{
    env,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::Result;
use tempfile::TempDir;

use crate::commands;

pub struct TestRepo {
    _temp_dir: TempDir,
    path: PathBuf,
}

impl TestRepo {
    pub fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().canonicalize()?;
        env::set_current_dir(&path)?;
        commands::init::run(&path)?;

        let test_repo = Self {
            _temp_dir: temp_dir,
            path,
        };
        Ok(test_repo)
    }

    pub fn file(self, relative_path: impl AsRef<Path>, contents: &str) -> Result<Self> {
        let file_path = self.path.join(relative_path.as_ref());
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut f = File::create(file_path)?;
        f.write_all(contents.as_bytes())?;
        f.sync_all()?;

        Ok(self)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
