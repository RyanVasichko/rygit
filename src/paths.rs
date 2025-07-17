use std::{
    env,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{Result, bail};

static REPOSITORY_ROOT_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn repository_root_path() -> PathBuf {
    REPOSITORY_ROOT_PATH
        .get_or_init(|| {
            let current_dir = env::current_dir().unwrap();
            discover_repository_root_from(current_dir)
                .expect("Failed to find repository root. Make sure you're in a rygit repository.")
        })
        .clone()
}

pub fn discover_repository_root_from(path: impl AsRef<Path>) -> Result<PathBuf> {
    let mut path = path.as_ref();

    loop {
        let rygit_path = path.join(".rygit");
        if rygit_path.exists() && rygit_path.is_dir() {
            return Ok(path.to_path_buf());
        } else {
            match path.parent() {
                Some(parent) => path = parent,
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

pub fn index_path() -> PathBuf {
    rygit_path().join("index")
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
    

    use anyhow::{Ok, Result};
    

    use crate::test_utils::setup_test_repository;

    use super::*;

    #[test]
    fn test_head_ref_path() -> Result<()> {
        let (repository_path, _temp_dir) = setup_test_repository()?;

        let expected = repository_path
            .join(".rygit")
            .join("refs")
            .join("heads")
            .join("master");
        assert_eq!(expected, head_ref_path());

        Ok(())
    }

    #[test]
    fn test_discover_root_paths_finds_rygit_dir() -> Result<()> {
        let (repository_path, _temp_dir) = setup_test_repository()?;

        assert_eq!(repository_path, repository_root_path());

        Ok(())
    }
}
