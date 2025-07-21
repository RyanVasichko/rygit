use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    iter::Peekable,
    path::{Path, PathBuf},
    str::FromStr,
    vec,
};

use anyhow::{Context, Result, bail};
use strum::{Display, EnumString};
use walkdir::WalkDir;

use crate::{
    compression::{compress, decompress},
    hash::Hash,
    index::Index,
    objects::{Object, blob::Blob, commit::Commit},
    paths::{head_ref_path, repository_root_path, rygit_path},
};

#[derive(Debug, Clone, PartialEq, Display, EnumString)]
pub enum EntryMode {
    #[strum(serialize = "100644")]
    File,
    #[strum(serialize = "40000")]
    Directory,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TreeEntry {
    object: Object,
    name: String,
}

// entry format:
// <mode> <file_name>\0<20 byte hash>
impl TreeEntry {
    pub fn create(path: impl AsRef<Path>, index: &Index) -> Result<Self> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .with_context(|| format!("Could not get file name for {}", path.display()))?
            .to_string_lossy()
            .to_string();
        if path.is_dir() {
            let directory_tree = Tree::create_recursive(path, index)?;
            let entry = TreeEntry {
                object: Object::Tree(directory_tree),
                name,
            };
            Ok(entry)
        } else if path.is_file() {
            let blob = Blob::create(path)?;
            let entry = TreeEntry {
                object: Object::Blob(blob),
                name,
            };
            Ok(entry)
        } else {
            bail!(
                "Unable to generate tree. {} Was neither a file nor a directory.",
                path.display()
            )
        }
    }

    pub fn object(&self) -> &Object {
        &self.object
    }

    pub fn hash(&self) -> &Hash {
        self.object.hash()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn parse(serialized_data_iter: &mut Peekable<vec::IntoIter<u8>>) -> Result<Self> {
        let mode: String = serialized_data_iter
            .take_while(|&c| c != b' ')
            .map(|c| c as char)
            .collect();
        let mode = EntryMode::from_str(&mode)
            .with_context(|| format!("Invalid tree entry. Invalid entry mode {mode}"))?;

        let name: String = serialized_data_iter
            .take_while(|&c| c != b'\0')
            .map(|c| c as char)
            .collect();

        let entry_object_hash_bytes: Vec<_> = serialized_data_iter.take(20).collect();
        let entry_object_hash = Hash::new(entry_object_hash_bytes.try_into().unwrap());
        let object_path = entry_object_hash.object_path();

        let object = match mode {
            EntryMode::File => {
                let blob = Blob::load(entry_object_hash.object_path())?;
                Object::Blob(blob)
            }
            EntryMode::Directory => {
                let tree = Tree::load(&object_path)?;
                Object::Tree(tree)
            }
        };

        let entry = Self { name, object };

        Ok(entry)
    }
}

// tree format:
// tree <content_length>\0<entries>
#[derive(Debug, PartialEq, Eq)]
pub struct Tree {
    hash: Hash,
    entries: Vec<TreeEntry>,
}

impl Tree {
    pub fn create(index: &Index) -> Result<Self> {
        let root = repository_root_path();
        Self::create_recursive(root, index)
    }

    fn create_recursive(path: impl AsRef<Path>, index: &Index) -> Result<Self> {
        let path = path.as_ref();
        let rygit_path = rygit_path();
        let directory_contents: Vec<_> = WalkDir::new(path)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_entry(|e| !e.path().starts_with(&rygit_path))
            .collect::<Result<_, _>>()
            .with_context(|| {
                format!(
                    "Unable to create tree. Unable to read directory contents for {}",
                    path.display()
                )
            })?;
        let mut entries: Vec<_> = directory_contents
            .iter()
            .map(|entry_path| TreeEntry::create(entry_path.path(), index))
            .collect::<Result<_, _>>()?;
        entries.sort_by(|a, b| a.name.cmp(&b.name));

        let serialized_data = serialize(&entries);
        let hash = Hash::of(&serialized_data);

        if !hash.object_path().exists() {
            let serialized_data = compress(&serialized_data)
                .context("Unable to generate tree. Unable to compress object.")?;
            fs::create_dir_all(hash.object_path().parent().unwrap())
                .and_then(|_| File::create(hash.object_path()))
                .and_then(|mut file| file.write_all(&serialized_data))
                .context("Unable to generate tree. Unable to create object file")?;
        }

        Ok(Self { hash, entries })
    }

    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    pub fn entries(&self) -> &[TreeEntry] {
        &self.entries
    }

    pub fn body(&self) -> Result<Vec<u8>> {
        let path = self.hash.object_path();
        let mut buf = vec![];
        File::open(path).unwrap().read_to_end(&mut buf).unwrap();
        let mut contents = decompress(&buf)?;
        if let Some(pos) = contents.iter().position(|&x| x == 0) {
            contents.drain(0..=pos);
        } else {
            bail!("Invalid blob header")
        }

        Ok(contents)
    }

    pub fn current() -> Result<Option<Self>> {
        let mut head_ref = String::new();
        File::open(head_ref_path())
            .and_then(|mut f| f.read_to_string(&mut head_ref))
            .context("Unable to read head ref")?;
        if head_ref.is_empty() {
            return Ok(None);
        }

        let head_ref_hash = Hash::from_hex(&head_ref)?;
        let head_commit = Commit::load(&head_ref_hash)?;
        let current_tree = head_commit.tree()?;
        Ok(Some(current_tree))
    }

    pub fn entries_flattened(&self) -> HashMap<PathBuf, Hash> {
        Tree::entries_flattened_recursive(self.entries(), repository_root_path())
    }

    fn entries_flattened_recursive(
        entries: &[TreeEntry],
        base_path: impl AsRef<Path>,
    ) -> HashMap<PathBuf, Hash> {
        let mut collected_entries = HashMap::new();
        let base_path = base_path.as_ref();
        for entry in entries {
            let full_path = base_path.join(&entry.name);
            match &entry.object {
                Object::Blob(blob) => {
                    collected_entries.insert(full_path, *blob.hash());
                }
                Object::Tree(tree) => {
                    let subtree_entries =
                        Tree::entries_flattened_recursive(tree.entries(), full_path);
                    collected_entries.extend(subtree_entries);
                }
            }
        }

        collected_entries
    }

    pub fn load(object_path: impl AsRef<Path>) -> Result<Self> {
        let mut serialized_data_buf = vec![];
        let serialized_data = File::open(&object_path)
            .and_then(|mut file| file.read_to_end(&mut serialized_data_buf))
            .map_err(anyhow::Error::from)
            .and_then(|_| decompress(&serialized_data_buf))
            .context("Unable to load tree. Unable to read object file")?;

        let hash = Hash::of(&serialized_data);
        let mut serialized_data_iter = serialized_data.into_iter().peekable();
        parse_header(&mut serialized_data_iter)?;

        let mut entries = vec![];
        while serialized_data_iter.peek().is_some() {
            let entry = TreeEntry::parse(&mut serialized_data_iter)?;
            entries.push(entry);
        }

        Ok(Tree { entries, hash })
    }

    pub fn find(&self, path: impl AsRef<Path>) -> Result<Option<&TreeEntry>> {
        let mut path = path.as_ref();
        let repository_root = repository_root_path();
        if path.starts_with(&repository_root) {
            path = path.strip_prefix(&repository_root)?;
        }
        let mut tree = self;

        let mut components = path.components().peekable();
        while let Some(component) = components.next() {
            let name = component.as_os_str().to_string_lossy();
            let entry = tree.entries.iter().find(|e| e.name == name);
            let entry = match entry {
                Some(e) => e,
                None => return Ok(None),
            };

            if components.peek().is_none() {
                match &entry.object {
                    Object::Blob(_) => return Ok(Some(entry)),
                    _ => return Ok(None),
                }
            }

            match &entry.object {
                Object::Tree(subtree) => tree = subtree,
                _ => return Ok(None),
            }
        }

        Ok(None)
    }
}

fn serialize(entries: &[TreeEntry]) -> Vec<u8> {
    let mut body: Vec<u8> = vec![];
    for entry in entries {
        let mode = match entry.object {
            Object::Blob(_) => EntryMode::File,
            Object::Tree(_) => EntryMode::Directory,
        };
        let entry_header = format!("{} {}\0", mode, entry.name);
        body.extend_from_slice(entry_header.as_bytes());
        body.extend_from_slice(entry.object.hash().as_bytes());
    }

    let mut serialized_data = format!("tree {}\0", body.len()).as_bytes().to_vec();
    serialized_data.extend_from_slice(&body);

    serialized_data
}

fn parse_header(serialized_data_iter: &mut Peekable<vec::IntoIter<u8>>) -> Result<()> {
    let label: String = serialized_data_iter
        .take_while(|&c| c != b' ')
        .map(|c| c as char)
        .collect();
    if label != "tree" {
        bail!("Invalid tree header. Must start with \"tree\"")
    }

    serialized_data_iter
        .take_while(|&c| c != b'\0')
        .for_each(drop);
    Ok(())
}

#[cfg(test)]
mod test {

    use anyhow::Result;

    use crate::test_utils::TestRepo;

    use super::*;

    #[test]
    fn test_from_index() -> Result<()> {
        let repo = TestRepo::new()?;
        repo.file("a.txt", "a")?
            .file("b.txt", "b")?
            .file("subdir1/c.txt", "c")?;

        let mut index = Index::load()?;
        index.add(repo.path().join("a.txt"))?;
        index.add(repo.path().join("b.txt"))?;
        index.add(repo.path().join("subdir1/c.txt"))?;

        let tree = Tree::create(&index)?;

        assert_eq!(3, tree.entries().len());
        let mut entries_iter = tree.entries().iter();

        let entry = entries_iter.next().unwrap();
        assert!(matches!(entry.object(), Object::Blob(_)));
        assert_eq!("a.txt", entry.name);

        let entry = entries_iter.next().unwrap();
        assert!(matches!(entry.object(), Object::Blob(_)));
        assert_eq!("b.txt", entry.name);

        let entry = entries_iter.next().unwrap();
        if let Object::Tree(subtree) = entry.object() {
            assert_eq!(1, subtree.entries().len());
            let entry = subtree.entries().first().unwrap();
            assert_eq!("c.txt", entry.name);
        } else {
            bail!(
                "Expected entry to be a tree but got {}",
                entry.object.as_ref()
            );
        }

        Ok(())
    }

    #[test]
    fn test_find() -> Result<()> {
        let repo = TestRepo::new()?;
        repo.file("a.txt", "a")?
            .file("subdir/a.txt", "a")?
            .file("a/a.txt", "a")?;
        let mut index = Index::load()?;
        index.add(repo.path().join("a.txt"))?;
        index.add(repo.path().join("a/a.txt"))?;
        index.add(repo.path().join("subdir/a.txt"))?;

        let tree = Tree::create(&index)?;

        assert!(tree.find("a.txt")?.is_some());
        assert!(tree.find("a/a.txt")?.is_some());
        assert!(tree.find("subdir/a.txt")?.is_some());
        assert!(tree.find("b.pdf")?.is_none());

        Ok(())
    }

    #[test]
    fn test_flattened() -> Result<()> {
        let repo = TestRepo::new()?;
        repo.file("a.txt", "a")?
            .file("a/a.txt", "b")?
            .file("b/b.txt", "b")?
            .stage(".")?
            .commit("Initial commit")?;
        let tree = Tree::current()?.unwrap();
        let flattened = tree.entries_flattened();

        assert_eq!(3, flattened.len());
        assert!(flattened.contains_key(&repo.path().join("a.txt")));
        assert!(flattened.contains_key(&repo.path().join("a/a.txt")));
        assert!(flattened.contains_key(&repo.path().join("b/b.txt")));

        Ok(())
    }
}
