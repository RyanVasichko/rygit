use std::{
    fs::{self, DirEntry},
    path::Path,
};

use anyhow::{Context, Result, anyhow};
use strum::{Display, EnumString};

use crate::{
    hash::Hash,
    objects::{Object, blob::Blob},
};

#[derive(Debug, Clone, PartialEq, Display, EnumString)]
pub enum EntryMode {
    #[strum(serialize = "100644")]
    File,
    #[strum(serialize = "40000")]
    Directory,
}

#[derive(Debug)]
pub struct TreeEntry {
    pub mode: EntryMode,
    pub object: Object,
    pub name: String,
}

#[derive(Debug)]
pub struct Tree {
    pub entries: Vec<TreeEntry>,
    pub serialized_data: Vec<u8>,
    pub hash: Hash,
}

impl Tree {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut entries: Vec<TreeEntry> = vec![];
        let dir_contents = fs::read_dir(path).with_context(|| {
            format!(
                "Unable to generate tree. Unable to read directory {}",
                path.display()
            )
        })?;
        let mut dir_contents: Vec<DirEntry> = dir_contents.collect::<Result<_, _>>()?;
        dir_contents.sort_by(|a, b| {
            a.file_name()
                .to_string_lossy()
                .cmp(&b.file_name().to_string_lossy())
        });
        for entry in dir_contents {
            let entry_path = entry.path();
            let name = entry_path
                .file_name()
                .with_context(|| format!("Could not get file name for {}", entry_path.display()))?
                .to_str()
                .with_context(|| {
                    format!("File name is not valid UTF-8 for {}", entry_path.display())
                })?
                .to_owned();
            if entry_path.is_dir() {
                let directory_tree = Tree::new(&entry_path)?;
                let entry = TreeEntry {
                    mode: EntryMode::Directory,
                    object: Object::Tree(directory_tree),
                    name,
                };
                entries.push(entry);
            } else if entry_path.is_file() {
                let blob = Blob::new(&entry_path)?;
                let entry = TreeEntry {
                    mode: EntryMode::File,
                    object: Object::Blob(blob),
                    name,
                };
                entries.push(entry);
            } else {
                return Err(anyhow!(
                    "Unable to generate tree. {} Was neither a file nor a directory.",
                    entry_path.display()
                ));
            }
        }
        let mut body: Vec<u8> = vec![];
        for entry in &entries {
            let entry_header = format!("{} {}\0", entry.mode, entry.name);
            body.extend_from_slice(entry_header.as_bytes());
            body.extend_from_slice(entry.object.hash().as_bytes());
        }

        let mut serialized_data = format!("tree {}\0", body.len()).as_bytes().to_vec();
        serialized_data.extend_from_slice(&body);
        let hash = Hash::of(&serialized_data);
        Ok(Self {
            serialized_data,
            hash,
            entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write, path::PathBuf};

    use anyhow::{Ok, Result};
    use tempfile::{NamedTempFile, TempDir};

    use crate::objects::blob::Blob;

    use super::*;

    fn assert_entry(entry: &TreeEntry, mode: EntryMode, name: &str, hash: &Hash) {
        assert_eq!(mode, entry.mode);
        assert_eq!(name, entry.name);
        assert_eq!(*hash, entry.object.hash());
    }

    fn create_file(dir_path: impl AsRef<Path>, name: &str, content: &[u8]) -> Result<PathBuf> {
        let file_path = dir_path.as_ref().join(name);
        let mut file = File::create(&file_path)?;
        file.write_all(content)?;
        Ok(file_path)
    }

    #[test]
    fn test_new_returns_an_error_when_path_is_not_a_directory() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let result = Tree::new(temp_file.path());

        assert!(result.is_err());

        let err = result.unwrap_err();
        let expected = format!(
            "Unable to generate tree. Unable to read directory {}",
            temp_file.path().display()
        );
        assert_eq!(expected, err.to_string());

        Ok(())
    }

    #[test]
    fn test_new_with_a_single_file_has_correct_entry() -> Result<()> {
        let dir = TempDir::new()?;
        let file_path = create_file(
            &dir,
            "test.txt",
            b"Karma police\narrest this man\nhe talks in math",
        )?;

        let tree = Tree::new(dir.path())?;
        let file_blob = Blob::new(file_path.as_path())?;

        assert_eq!(1, tree.entries.len());

        let entry = tree.entries.first().unwrap();
        assert_entry(entry, EntryMode::File, "test.txt", &file_blob.hash);

        Ok(())
    }

    #[test]
    fn test_new_with_a_subdirectory_has_correct_entry() -> Result<()> {
        let dir = TempDir::new()?;
        let subdir_path = dir.path().join("subdir");
        fs::create_dir(&subdir_path)?;
        let tree = Tree::new(dir.path())?;

        let subdir_tree = Tree::new(&subdir_path)?;

        assert_eq!(1, tree.entries.len());

        let entry = tree.entries.first().unwrap();
        assert_entry(entry, EntryMode::Directory, "subdir", &subdir_tree.hash);

        Ok(())
    }

    #[test]
    fn test_new_with_files_and_subdirectories_has_correct_entries() -> Result<()> {
        let dir = TempDir::new()?;
        let file1_path = create_file(
            &dir,
            "a.txt",
            b"Karma police\narrest this man\nhe talks in math",
        )?;
        let subdir1_path = dir.path().join("b");
        fs::create_dir(&subdir1_path)?;
        let file2_path = create_file(
            &dir,
            "y.txt",
            b"Because we separate\nLike ripples on a blank shore",
        )?;
        let subdir2_path = dir.path().join("z");
        fs::create_dir(&subdir2_path)?;

        let tree = Tree::new(&dir)?;
        assert_eq!(4, tree.entries.len());
        let entry1_blob = Blob::new(file1_path)?;
        assert_entry(
            tree.entries.first().unwrap(),
            EntryMode::File,
            "a.txt",
            &entry1_blob.hash,
        );
        let entry2_tree = Tree::new(&subdir1_path)?;
        assert_entry(
            tree.entries.get(1).unwrap(),
            EntryMode::Directory,
            "b",
            &entry2_tree.hash,
        );
        let entry3_blob = Blob::new(file2_path)?;
        assert_entry(
            tree.entries.get(2).unwrap(),
            EntryMode::File,
            "y.txt",
            &entry3_blob.hash,
        );
        let entry4_tree = Tree::new(subdir2_path)?;
        assert_entry(
            tree.entries.get(3).unwrap(),
            EntryMode::Directory,
            "z",
            &entry4_tree.hash,
        );

        Ok(())
    }

    #[test]
    fn test_new_generates_correct_contents() -> Result<()> {
        let dir = TempDir::new()?;
        let file1_path = create_file(
            &dir,
            "a.txt",
            b"Karma police\narrest this man\nhe talks in math",
        )?;
        let subdir1_path = dir.path().join("b_dir");
        fs::create_dir(&subdir1_path)?;
        let file2_path = create_file(
            &dir,
            "c.txt",
            b"Because we separate\nLike ripples on a blank shore",
        )?;
        let subdir2_path = dir.path().join("d_dir");
        fs::create_dir(&subdir2_path)?;

        let blob1 = Blob::new(file1_path)?;
        let tree1 = Tree::new(&subdir1_path)?;
        let blob2 = Blob::new(file2_path)?;
        let tree2 = Tree::new(&subdir2_path)?;
        let tree = Tree::new(dir.path())?;

        let expected_body = [
            format!("{} {}\0", EntryMode::File, "a.txt").as_bytes(),
            blob1.hash.as_bytes(),
            format!("{} {}\0", EntryMode::Directory, "b_dir").as_bytes(),
            tree1.hash.as_bytes(),
            format!("{} {}\0", EntryMode::File, "c.txt").as_bytes(),
            blob2.hash.as_bytes(),
            format!("{} {}\0", EntryMode::Directory, "d_dir").as_bytes(),
            tree2.hash.as_bytes(),
        ]
        .concat();
        let expected_contents = [
            format!("tree {}\0", expected_body.len()).as_bytes(),
            &expected_body,
        ]
        .concat();

        assert_eq!(expected_contents, tree.serialized_data);

        Ok(())
    }
}
