use std::{env, fs::File, io::Read, path::PathBuf};

pub fn repository_root_path() -> PathBuf {
    env::current_dir().unwrap()
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
}
