use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

use anyhow::{Context, Result, anyhow};

pub fn run(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let rygit_dir = path.join(".rygit");
    if rygit_dir.exists() {
        return Err(anyhow!("rygit already initialized"));
    }

    fs::create_dir(&rygit_dir)
        .context("Unable to initialize rygit, unable to create .rygit directory")?;

    File::create(rygit_dir.join("HEAD"))
        .context("Unable to initialize rygit, unable to create .rygit/HEAD")?
        .write_all(b"ref: refs/heads/master")?;

    let refs_path = rygit_dir.join("refs");
    fs::create_dir(&refs_path)
        .context("Unable to initialize rygit, unable to create .rygit/refs directory")?;

    fs::create_dir(refs_path.join("heads"))
        .context("Unable to initialize rygit, unable to create .rygit/refs/heads directory")?;

    File::create(refs_path.join("heads").join("master"))
        .context("Unable to initialize rygit. Unable to create refs/heads/master")?;

    println!("Repository initialized!");

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use anyhow::{Ok, Result};
    use serial_test::serial;
    use tempfile::TempDir;

    use super::*;

    #[test]
    #[serial]
    fn test_run_when_already_initialized() -> Result<()> {
        let dir = TempDir::new()?;
        env::set_current_dir(&dir)?;
        fs::create_dir(dir.path().join(".rygit"))?;
        let result = run(dir);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    #[serial]
    fn test_run_initializes_ryigit() -> Result<()> {
        let dir = TempDir::new()?;

        run(&dir)?;

        let rygit_path = dir.path().join(".rygit");
        let rygit_initialized = rygit_path.exists() && rygit_path.is_dir();
        assert!(rygit_initialized);

        let head_path = rygit_path.join("HEAD");
        let head_initialized = head_path.exists() && head_path.is_file();
        assert!(head_initialized);
        let head_contents = fs::read_to_string(&head_path)?;
        assert_eq!("ref: refs/heads/master", head_contents);

        let refs_path = rygit_path.join("refs");
        let refs_initialized = refs_path.exists() && refs_path.is_dir();
        assert!(refs_initialized);

        let heads_path = refs_path.join("heads");
        let heads_initialized = heads_path.exists() && heads_path.is_dir();
        assert!(heads_initialized);

        Ok(())
    }
}
