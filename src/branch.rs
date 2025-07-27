use std::fs;

use anyhow::{Context, Ok, Result, bail};
use walkdir::WalkDir;

use crate::{
    hash::Hash,
    objects::{blob::Blob, commit::Commit},
    paths::{head_path, head_ref_path, refs_path, repository_root_path, rygit_path},
};

pub struct Branch {
    name: String,
    commit_hash: Hash,
}

impl Branch {
    pub fn current() -> Result<Self> {
        let head = fs::read_to_string(head_path()).context("Unable to read head")?;
        let name = head
            .strip_prefix("ref: refs/heads/")
            .with_context(|| format!("Invalid head ref {head}"))?
            .to_string();
        let head_ref = fs::read_to_string(head_ref_path()).context("Unable to read head ref")?;
        let commit_hash = Hash::from_hex(&head_ref)
            .context("Unable to determine branch commit hash. Invalid format")?;
        let branch = Self { name, commit_hash };

        Ok(branch)
    }

    pub fn create(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        let commit_hash = Branch::current()?.commit_hash;
        // TODO: What to do if branch already exists?
        let ref_file_path = refs_path().join("heads").join(&name);
        fs::write(ref_file_path, commit_hash.to_hex())
            .context("Unable to create branch. Unable to write ref file")?;
        let branch = Self { name, commit_hash };
        Ok(branch)
    }

    pub fn find_by_name(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        let ref_path = refs_path().join("heads").join(&name);
        if !ref_path.exists() {
            bail!("{name} not a branch");
        }

        let commit_hash = fs::read_to_string(&ref_path).context("Unable to read branch ref")?;
        let commit_hash = Hash::from_hex(&commit_hash)
            .context("Unable to load branch. Commit hash is not a valid hash")?;

        Ok(Self { name, commit_hash })
    }

    pub fn list() -> Result<Vec<Branch>> {
        let branches_path = refs_path().join("heads");
        let branches: Vec<_> = WalkDir::new(&branches_path)
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| e.path().is_file())
            .map(|e| {
                let e = e?;
                let path = e.path();
                let name = path
                    .strip_prefix(&branches_path)?
                    .to_string_lossy()
                    .to_string();
                let commit_hash = fs::read_to_string(path)?;
                let commit_hash = Hash::from_hex(&commit_hash)?;

                Ok(Self { name, commit_hash })
            })
            .collect::<Result<_, _>>()?;

        Ok(branches)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn switch(name: impl Into<String>) -> Result<()> {
        let name = name.into();
        let directory_contents =
            fs::read_dir(repository_root_path()).context("Unable to read repository contents")?;
        let rygit_path = rygit_path();
        for entry in directory_contents {
            let entry = entry.context("Unable to read repository contents")?;
            let path = entry.path();
            if path.starts_with(&rygit_path) {
                continue;
            }

            if path.is_file() {
                fs::remove_file(&path)
                    .with_context(|| format!("Unable to remove file {}", path.display()))?;
            } else if path.is_dir() {
                fs::remove_dir_all(&path)
                    .with_context(|| format!("Unable to remove directory {}", path.display()))?;
            }
        }

        let branch = Branch::find_by_name(&name)?;
        let commit = branch.commit()?;
        let tree = commit.tree()?;
        for (entry_path, entry_hash) in tree.entries_flattened() {
            let blob = Blob::load(entry_hash.object_path())?;
            let body = blob.body()?.iter().map(|&c| c as char).collect::<String>();
            if let Some(parent) = entry_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("unable to create file {}", entry_path.display()))?;
            }
            fs::write(entry_path, body)?;
        }

        fs::write(head_path(), format!("ref: refs/heads/{name}"))?;

        Ok(())
    }

    fn commit(&self) -> Result<Commit> {
        Commit::load(&self.commit_hash)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Ok;

    use crate::test_utils::TestRepo;

    use super::*;

    #[test]
    fn test_current() -> Result<()> {
        let repo = TestRepo::new()?;
        let branch = Branch::current();
        assert!(branch.is_err());

        repo.file("a.txt", "a")?
            .stage(".")?
            .commit("Initial commit")?
            .branch("test")?;
        let branch = Branch::current()?;
        assert_eq!("master", branch.name);

        Ok(())
    }

    #[test]
    fn test_create() -> Result<()> {
        let repo = TestRepo::new()?;
        let branch = Branch::create("test");
        assert!(branch.is_err());

        repo.file("a.txt", "a")?
            .stage(".")?
            .commit("Initial commit")?
            .branch("test")?;
        let initial_commit_hash = fs::read_to_string(head_ref_path())?;
        let initial_commit_hash = Hash::from_hex(&initial_commit_hash)?;

        repo.file("b.txt", "b")?
            .stage(".")?
            .commit("Second commit")?;
        let second_commit_hash = fs::read_to_string(head_ref_path())?;
        let second_commit_hash = Hash::from_hex(&second_commit_hash)?;

        let test_branch = Branch::find_by_name("test")?;
        assert_eq!("test", test_branch.name);
        assert_eq!(initial_commit_hash, test_branch.commit_hash);

        let master_branch = Branch::find_by_name("master")?;
        assert_eq!("master", master_branch.name);
        assert_eq!(second_commit_hash, master_branch.commit_hash);

        let branches = Branch::list()?;
        assert_eq!(2, branches.len());
        assert!(branches.iter().any(|b| b.name == "master"));
        assert!(branches.iter().any(|b| b.name == "test"));

        Ok(())
    }

    #[test]
    fn test_switch() -> Result<()> {
        let repo = TestRepo::new()?;
        repo.file("a.txt", "a")?
            .file("a/a.txt", "subdira")?
            .stage(".")?
            .commit("Initial commit")?;

        repo.branch("test")?
            .switch("test")?
            .file("b.txt", "b")?
            .file("b/b.txt", "subdirb")?
            .stage(".")?
            .commit("Commit on test")?;

        let file_b_path = repo.path().join("b.txt");
        assert_eq!("test", Branch::current()?.name);
        assert!(file_b_path.exists());
        let subdir_file_b_path = repo.path().join("b").join("b.txt");
        assert!(subdir_file_b_path.exists());
        assert_eq!("subdirb", fs::read_to_string(subdir_file_b_path)?);

        repo.switch("master")?;
        assert_eq!("master", Branch::current()?.name);
        assert!(!file_b_path.exists());
        assert_eq!("a", fs::read_to_string(repo.path().join("a.txt"))?);
        let subdir_file_a_path = repo.path().join("a").join("a.txt");
        assert!(subdir_file_a_path.exists());
        assert_eq!("subdira", fs::read_to_string(subdir_file_a_path)?);

        repo.switch("test")?;
        assert_eq!("test", Branch::current()?.name);
        assert!(file_b_path.exists());
        assert_eq!("b", fs::read_to_string(&file_b_path)?);
        assert_eq!("a", fs::read_to_string(repo.path().join("a.txt"))?);

        // TODO: Test for handling uncommitted files

        Ok(())
    }
}
