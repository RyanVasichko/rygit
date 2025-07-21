use std::fs;

use anyhow::{Context, Ok, Result};
use walkdir::WalkDir;

use crate::{
    hash::Hash,
    paths::{head_path, head_ref_path, refs_path},
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
            .commit("Initial commit")?;
        let branch = Branch::current();
        assert!(branch.is_ok());

        Ok(())
    }

    #[test]
    fn test_create() -> Result<()> {
        let repo = TestRepo::new()?;
        let branch = Branch::create("test");
        assert!(branch.is_err());

        repo.file("a.txt", "a")?
            .stage(".")?
            .commit("Initial commit")?;
        let branch = Branch::create("test")?;
        assert_eq!("test", branch.name);

        let branches = Branch::list()?;
        assert_eq!(2, branches.len());
        assert!(branches.iter().any(|b| b.name == "master"));
        assert!(branches.iter().any(|b| b.name == "test"));

        Ok(())
    }
}
