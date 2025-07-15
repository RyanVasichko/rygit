use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

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
        self.add_recursive(path)?;
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
        if !path.is_file() {
            bail!("Unable to add {}. Not a file", path.display());
        }

        let blob = Blob::create(path)?;
        let position = self.files.iter().position(|f| f.path == path);
        let index_file = IndexFile {
            path: path.to_path_buf(),
            hash: *blob.hash(),
        };
        if let Some(position) = position {
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
        if path == rygit_path || path.starts_with(rygit_path) {
            return Ok(());
        }

        let readdir = path.read_dir().with_context(|| {
            format!("Unable to add {}. Unable to read directory", path.display())
        })?;
        for file in readdir {
            let file = file.with_context(|| {
                format!("Unable to add {}. Unable to read file", path.display())
            })?;
            self.add(file.path())?;
        }

        Ok(())
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
    use std::{
        env,
        fs::{self, File},
        io::Write,
    };

    use tempfile::TempDir;

    use anyhow::{Ok, Result};

    use crate::commands;

    use super::*;

    #[test]
    fn test_add() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let repository_path = temp_dir.path().canonicalize()?;
        commands::init::run(&repository_path)?;
        env::set_current_dir(&temp_dir)?;

        let file_a_path = repository_path.join("a.txt");
        File::create(&file_a_path)?.write_all(b"a")?;
        let file_a_path = file_a_path.canonicalize()?;

        let file_b_path = repository_path.join("b.txt");
        File::create(&file_b_path)?.write_all(b"b")?;

        let subdir1_path = repository_path.join("subdir1");
        fs::create_dir_all(&subdir1_path)?;

        let file_c_path = subdir1_path.join("c.txt");
        File::create(&file_c_path)?.write_all(b"c")?;

        let subdir2_path = repository_path.join("subdir2");
        fs::create_dir_all(&subdir2_path)?;

        let file_d_path = subdir2_path.join("d.txt");
        File::create(&file_d_path)?.write_all(b"d")?;
        let file_e_path = subdir2_path.join("e.txt");
        File::create(&file_e_path)?.write_all(b"e")?;

        let mut index = Index::load()?;
        index.add(&file_a_path)?;

        assert_eq!(1, index.files.len());
        let indexed_file_paths: HashSet<_> = index.files.iter().map(|f| &f.path).collect();
        assert!(indexed_file_paths.contains(&file_a_path));

        let mut index = Index::load()?;
        assert!(indexed_file_paths.contains(&file_a_path));

        index.add(subdir1_path)?;
        index.add(&file_e_path)?;
        assert_eq!(3, index.files.len());
        let mut files_iter = index.files.iter();
        assert_eq!(file_a_path, files_iter.next().unwrap().path);
        assert_eq!(file_c_path, files_iter.next().unwrap().path);
        assert_eq!(file_e_path, files_iter.next().unwrap().path);

        Ok(())
    }
}
