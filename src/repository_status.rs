use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context, Result};
use strum::Display;
use walkdir::WalkDir;

use crate::{
    index::Index,
    objects::{blob::Blob, tree::Tree},
    paths::{repository_root_path, rygit_path},
};

#[derive(Debug, PartialEq, Eq, Display)]
pub enum FileStatus {
    Deleted,
    Modified,
    Added,
}

#[derive(Debug, PartialEq, Eq)]
pub struct StatusEntry {
    pub path: PathBuf,
    pub status: FileStatus,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RepositoryStatus {
    staged_changes: Vec<StatusEntry>,
    unstaged_changes: Vec<StatusEntry>,
    untracked_files: Vec<PathBuf>,
}

impl RepositoryStatus {
    pub fn load() -> Result<Self> {
        let committed_tree = Tree::current()?;
        let committed_tree_files = if let Some(committed_tree) = committed_tree {
            committed_tree.entries_flattened()
        } else {
            HashMap::new()
        };

        let rygit_path = rygit_path();
        let working_tree_file_paths: Vec<_> = WalkDir::new(repository_root_path())
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| !e.path().starts_with(&rygit_path))
            .collect::<Result<_, _>>()
            .context("Unable to read repository contents")?;
        let mut working_tree_files = HashMap::new();
        for entry in working_tree_file_paths {
            let entry_path = entry.path();
            let entry_blob_hash = Blob::hash_for(entry_path)?;
            working_tree_files.insert(entry_path.to_path_buf(), entry_blob_hash);
        }

        let mut staged_files = HashMap::new();
        let index = Index::load()?;
        for index_file in index.files() {
            staged_files.insert(index_file.path().to_path_buf(), *index_file.hash());
        }

        let mut untracked_files = vec![];
        let mut unstaged_changes = vec![];
        let mut staged_changes = vec![];

        for committed_tree_file in committed_tree_files.iter() {
            let committed_tree_file_path = committed_tree_file.0;
            let staged_file_hash = staged_files.get(committed_tree_file_path);
            if staged_file_hash.is_none() {
                staged_changes.push(StatusEntry {
                    path: committed_tree_file_path.to_path_buf(),
                    status: FileStatus::Deleted,
                });
            }

            if staged_file_hash.is_some_and(|h| h != committed_tree_file.1) {
                staged_changes.push(StatusEntry {
                    path: committed_tree_file_path.to_path_buf(),
                    status: FileStatus::Modified,
                });
            }
        }

        for staged_file in &staged_files {
            let staged_file_path = staged_file.0;
            if !committed_tree_files.contains_key(staged_file_path) {
                staged_changes.push(StatusEntry {
                    path: staged_file_path.to_path_buf(),
                    status: FileStatus::Added,
                });
            }

            if !working_tree_files.contains_key(staged_file_path) {
                unstaged_changes.push(StatusEntry {
                    path: staged_file_path.to_path_buf(),
                    status: FileStatus::Deleted,
                });
            }
        }

        for working_tree_file in &working_tree_files {
            let working_tree_file_path = working_tree_file.0;
            let staged_file_hash = staged_files.get(working_tree_file_path);
            if staged_file_hash.is_none() {
                untracked_files.push(working_tree_file_path.clone());
            }

            if staged_file_hash.is_some_and(|h| h != working_tree_file.1) {
                unstaged_changes.push(StatusEntry {
                    path: working_tree_file_path.to_path_buf(),
                    status: FileStatus::Modified,
                });
            }
        }

        staged_changes.sort_by(|a, b| a.path.cmp(&b.path));
        unstaged_changes.sort_by(|a, b| a.path.cmp(&b.path));
        untracked_files.sort();

        let status = Self {
            staged_changes,
            unstaged_changes,
            untracked_files,
        };
        Ok(status)
    }

    pub fn unstaged_changes(&self) -> &[StatusEntry] {
        &self.unstaged_changes
    }

    pub fn staged_changes(&self) -> &[StatusEntry] {
        &self.staged_changes
    }

    pub fn untracked_files(&self) -> &[PathBuf] {
        &self.untracked_files
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;

    use crate::test_utils::TestRepo;

    use super::*;

    fn test_untracked_files(with_initial_commit: bool) -> Result<()> {
        let repo = TestRepo::new()?;
        repo.file("a.txt", "a")?;

        if with_initial_commit {
            repo.file("b.txt", "b")?
                .stage("b.txt")?
                .commit("Initial commit")?;
        }

        let status = RepositoryStatus::load()?;
        assert_eq!(1, status.untracked_files.len());
        let untracked_file = status.untracked_files.first().unwrap();
        assert_eq!(repo.path().join("a.txt"), untracked_file.as_path());

        let _repo = repo.stage("a.txt")?;
        let status = RepositoryStatus::load()?;
        assert_eq!(0, status.untracked_files.len());

        Ok(())
    }

    #[test]
    fn test_untracked_files_no_commits() -> Result<()> {
        test_untracked_files(false)
    }

    #[test]
    fn test_untracked_files_with_commits() -> Result<()> {
        test_untracked_files(true)
    }

    fn test_staged_files(with_initial_commit: bool) -> Result<()> {
        let repo = TestRepo::new()?;
        if with_initial_commit {
            repo.file("bogus.txt", "bogus")?
                .stage(".")?
                .commit("Initial commit")?;
        }
        repo.file("a.txt", "a")?.stage(".")?;

        let status = RepositoryStatus::load()?;
        assert_eq!(1, status.staged_changes.len());
        let staged_file = status.staged_changes.first().unwrap();
        let expected = StatusEntry {
            path: repo.path().join("a.txt"),
            status: FileStatus::Added,
        };
        assert_eq!(&expected, staged_file);

        repo.commit("Commit 1")?.file("a.txt", "b")?.stage(".")?;
        let status = RepositoryStatus::load()?;
        assert_eq!(1, status.staged_changes.len());
        let staged_file = status.staged_changes.first().unwrap();
        let expected = StatusEntry {
            path: repo.path().join("a.txt"),
            status: FileStatus::Modified,
        };
        assert_eq!(&expected, staged_file);

        repo.commit("Commit 2")?.remove_file("a.txt")?.stage(".")?;
        let status = RepositoryStatus::load()?;
        assert_eq!(1, status.staged_changes.len());
        let staged_file = status.staged_changes.first().unwrap();
        let expected = StatusEntry {
            path: repo.path().join("a.txt"),
            status: FileStatus::Deleted,
        };
        assert_eq!(&expected, staged_file);

        // todo!("Different status kinds: deleted, modified, new file");
        Ok(())
    }

    #[test]
    fn test_staged_changes_no_commits() -> Result<()> {
        test_staged_files(false)
    }

    #[test]
    fn test_staged_changes_with_commits() -> Result<()> {
        test_staged_files(true)
    }

    #[test]
    fn test_unstaged_changes() -> Result<()> {
        let repo = TestRepo::new()?;
        repo.file("a.txt", "a")?
            .file("b.txt", "b")?
            .stage(".")?
            .commit("Initial commit")?
            .file("a.txt", "b")?
            .remove_file("b.txt")?
            .file("c.txt", "c")?;

        let status = RepositoryStatus::load()?;
        let expected = vec![
            StatusEntry {
                path: repo.path().join("a.txt"),
                status: FileStatus::Modified,
            },
            StatusEntry {
                path: repo.path().join("b.txt"),
                status: FileStatus::Deleted,
            },
        ];
        assert_eq!(expected, status.unstaged_changes);

        Ok(())
    }

    #[test]
    fn test_clean_repo() -> Result<()> {
        let _repo = TestRepo::new()?;
        let status = RepositoryStatus::load()?;

        assert!(status.staged_changes.is_empty());
        assert!(status.unstaged_changes.is_empty());
        assert!(status.untracked_files.is_empty());

        Ok(())
    }
}
