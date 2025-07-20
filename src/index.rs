use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

use crate::{
    hash::Hash,
    objects::blob::Blob,
    paths::{index_path, repository_root_path, rygit_path},
};

pub struct Index {
    files: Vec<IndexFile>,
}

impl Index {
    pub fn load() -> Result<Self> {
        let repository_path = repository_root_path();
        let file = File::open(index_path()).context("Unable to open index file")?;
        let reader = BufReader::new(file);
        let mut files = vec![];
        for line in reader.lines() {
            let line = line.context("Unable to read index file")?;
            let mut parts = line.split(" ");
            let relative_path = parts
                .next()
                .context("Unable to load index. Invalid index format. Relative path missing")?;
            let path = repository_path.join(relative_path);
            let hash = parts
                .next()
                .context("Unable to load index. Invalid index format. Invalid hash")?;
            let hash = Hash::from_hex(hash)
                .context("Unable to load index. Invalid index format. Invalid hash")?;
            files.push(IndexFile { path, hash });
        }

        Ok(Self { files })
    }

    pub fn add(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        self.add_recursive(path)?;
        if path.is_dir() {
            self.remove_deleted_files(path);
        }
        self.files.sort_by(|a, b| a.path.cmp(&b.path));
        self.write()
    }

    fn add_recursive(&mut self, path: impl AsRef<Path>) -> Result<()> {
        if path.as_ref().is_dir() {
            self.add_dir(path)
        } else {
            self.add_file(path)
        }
    }

    fn add_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let file_position = self.files.iter().position(|f| f.path == path);

        if !path.exists() {
            if let Some(pos) = file_position.as_ref() {
                self.files.remove(*pos);
                return Ok(());
            } else {
                let relative_path = path.strip_prefix(repository_root_path())?;
                bail!(
                    "Unable to add {}. Did not match any files",
                    relative_path.display()
                )
            }
        }

        let blob = Blob::create(path)?;
        let index_file = IndexFile {
            path: path.to_path_buf(),
            hash: *blob.hash(),
        };
        if let Some(position) = file_position {
            self.files[position] = index_file;
        } else {
            self.files.push(index_file);
        }

        Ok(())
    }

    fn add_dir(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if !path.is_dir() {
            bail!("Unable to add {}. Not a dir", path.display());
        }

        let rygit_path = rygit_path();
        let entries = WalkDir::new(path)
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| !e.path().starts_with(&rygit_path));
        for entry in entries {
            let entry = entry.with_context(|| {
                format!("Unable to add {}. Unable to read file", path.display())
            })?;
            self.add_recursive(entry.path())?
        }

        Ok(())
    }

    fn remove_deleted_files(&mut self, path: &Path) {
        self.files.retain(|f| {
            if !f.path.starts_with(path) {
                return true;
            }

            f.path.exists()
        });
    }

    fn write(&self) -> Result<()> {
        let repository_path = repository_root_path().canonicalize()?;
        let mut index_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(index_path())
            .context("Unable to write index contents. Unable to open index file")?;

        for file in self.files.iter() {
            let path = &file.path;
            let relative_path = path.strip_prefix(&repository_path).with_context(|| {
                format!(
                    "Unable to write index file. Unable to determine relative path for {} with base path {}",
                    path.display(),
                    repository_path.display()
                )
            })?;
            let line = format!("{} {}\n", relative_path.display(), file.hash.to_hex());
            index_file
                .write_all(line.as_bytes())
                .context("Unable to write to index file")?;
        }

        Ok(())
    }

    pub fn indexed_files_in_directory(&self, path: impl AsRef<Path>) -> Vec<PathBuf> {
        let path = path.as_ref();
        self.files
            .iter()
            .filter(|f| f.path.is_file() && f.path.parent().unwrap() == path)
            .map(|f| f.path.to_path_buf())
            .collect()
    }

    pub fn indexed_directories_in_directory(&self, path: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
        let path = path.as_ref();
        let mut indexed_directories = HashSet::new();
        for file in self.files.iter() {
            let file_path = &file.path;
            if file_path.parent().is_none() {
                continue;
            }
            let parent = file_path.parent().unwrap();
            if parent.starts_with(path) && parent != path {
                let subdir_path = parent.strip_prefix(path)?.components().next().unwrap();
                indexed_directories.insert(path.join(subdir_path));
            }
        }

        let indexed_directories = indexed_directories.into_iter().collect();
        Ok(indexed_directories)
    }

    pub fn files(&self) -> &Vec<IndexFile> {
        &self.files
    }
}

pub struct IndexFile {
    path: PathBuf,
    hash: Hash,
}

impl IndexFile {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn hash(&self) -> &Hash {
        &self.hash
    }
}

#[cfg(test)]
mod tests {

    use anyhow::{Ok, Result};

    use crate::test_utils::TestRepo;

    use super::*;

    #[test]
    fn test_add() -> Result<()> {
        let repo = TestRepo::new()?;
        repo.file("a.txt", "a")?
            .file("b.txt", "b")?
            .file("subdir1/c.txt", "c")?
            .file("subdir2/d.txt", "d")?
            .file("subdir2/e.txt", "e")?;

        let mut index = Index::load()?;
        index.add(repo.path().join("a.txt"))?;

        assert_eq!(1, index.files.len());
        let indexed_file_paths: HashSet<_> = index.files.iter().map(|f| &f.path).collect();
        assert!(indexed_file_paths.contains(&repo.path().join("a.txt")));

        let mut index = Index::load()?;
        assert!(indexed_file_paths.contains(&repo.path().join("a.txt")));

        index.add(repo.path().join("subdir1"))?;
        index.add(repo.path().join("subdir2/e.txt"))?;
        assert_eq!(3, index.files.len());
        let mut files_iter = index.files.iter();
        assert_eq!(repo.path().join("a.txt"), files_iter.next().unwrap().path);
        assert_eq!(
            repo.path().join("subdir1/c.txt"),
            files_iter.next().unwrap().path
        );
        assert_eq!(
            repo.path().join("subdir2/e.txt"),
            files_iter.next().unwrap().path
        );

        repo.remove_file("a.txt")?
            .remove_file("subdir1/c.txt")?
            .stage(".")?;
        let index = Index::load()?;
        let file_a_present = index
            .files()
            .iter()
            .any(|f| f.path == repo.path().join("a.txt"));
        assert!(!file_a_present);

        let file_c_present = index
            .files()
            .iter()
            .any(|f| f.path == repo.path().join("subdir1/c.txt"));
        assert!(!file_c_present);

        Ok(())
    }
}
