use std::fs;

use anyhow::{Context, Result};

use crate::{
    hash::Hash,
    paths::{head_path, head_ref_path},
};

pub struct Branch {
    name: String,
    commit_hash: Option<Hash>,
}

impl Branch {
    pub fn current() -> Result<Self> {
        let head = fs::read_to_string(head_path()).context("Unable to read head")?;
        let name = head
            .strip_prefix("ref: refs/heads/")
            .with_context(|| format!("Invalid head ref {head}"))?
            .to_string();
        let head_ref = fs::read_to_string(head_ref_path()).context("Unable to read head ref")?;
        let commit_hash = if head_ref.is_empty() {
            None
        } else {
            let hash = Hash::from_hex(&head_ref)
                .context("Unable to determine branch commit hash. Invalid format")?;
            Some(hash)
        };
        let branch = Self { name, commit_hash };

        Ok(branch)
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
        let branch = Branch::current()?;
        assert_eq!("master", branch.name);
        assert!(branch.commit_hash.is_none());

        repo.file("a.txt", "a")?
            .stage(".")?
            .commit("Initial commit")?;
        let branch = Branch::current()?;
        assert!(branch.commit_hash.is_some());

        Ok(())
    }
}
