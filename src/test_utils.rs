use std::{env, path::PathBuf};

use anyhow::Result;
use tempfile::TempDir;

use crate::commands;

pub fn setup_test_repository() -> Result<(PathBuf, TempDir)> {
    let dir = TempDir::new()?;
    let repository_path = dir.path().canonicalize()?;
    env::set_current_dir(&repository_path)?;
    commands::init::run(&repository_path)?;

    Ok((repository_path, dir))
}
