use std::{env, fs::File, io::Read, path::PathBuf, sync::OnceLock};

use anyhow::{Result, bail};

static REPOSITORY_ROOT_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn repository_root_path() -> PathBuf {
    REPOSITORY_ROOT_PATH
        .get_or_init(|| {
            discover_repository_root()
                .expect("Failed to find repository root. Make sure you're in a rygit repository.")
        })
        .clone()
}

fn discover_repository_root() -> Result<PathBuf> {
    let mut current_dir = env::current_dir()?;

    loop {
        let rygit_path = current_dir.join(".rygit");
        if rygit_path.exists() && rygit_path.is_dir() {
            return Ok(current_dir);
        } else {
            match current_dir.parent() {
                Some(parent) => current_dir = parent.to_path_buf(),
                None => bail!("Not in a rygit repository (or any parent directories)"),
            }
        }
    }
}

pub fn rygit_path() -> PathBuf {
    repository_root_path().join(".rygit")
}

pub fn objects_path() -> PathBuf {
    rygit_path().join("objects")
}

pub fn refs_path() -> PathBuf {
    rygit_path().join("refs")
}

pub fn head_path() -> PathBuf {
    rygit_path().join("HEAD")
}

pub fn head_ref_path() -> PathBuf {
    let mut head_contents = vec![];
    File::open(head_path())
        .unwrap()
        .read_to_end(&mut head_contents)
        .unwrap();

    if !head_contents.starts_with(b"ref: ") {
        panic!("Invaild format for HEAD")
    }

    head_contents.drain(0..5).for_each(drop);
    let head_contents: String = head_contents.into_iter().map(|c| c as char).collect();
    rygit_path().join(head_contents.trim())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use anyhow::{Ok, Result};
    use tempfile::TempDir;

    use crate::commands::init;

    use super::*;

    #[test]
    fn test_head_ref_path() -> Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().canonicalize().unwrap();
        env::set_current_dir(&path)?;
        init::run(&dir)?;

        let expected = path
            .join(".rygit")
            .join("refs")
            .join("heads")
            .join("master");
        assert_eq!(expected, head_ref_path());

        Ok(())
    }

    #[test]
    fn test_discover_root_paths_finds_rygit_dir() -> Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path();
        let subdir_path = path.join("subdir");
        fs::create_dir_all(&subdir_path)?;

        init::run(&dir)?;
        env::set_current_dir(subdir_path)?;

        let repository_root_path = repository_root_path().canonicalize()?;
        assert_eq!(path.canonicalize()?, repository_root_path);

        Ok(())
    }
}
