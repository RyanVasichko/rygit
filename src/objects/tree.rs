use std::{
    fs::{self, DirEntry, File},
    io::{Read, Write},
    iter::Peekable,
    path::Path,
    str::FromStr,
    vec,
};

use anyhow::{Context, Result, bail};
use strum::{Display, EnumString};

use crate::{
    compression::{compress, decompress},
    hash::Hash,
    objects::{Object, blob::Blob},
    paths::rygit_path,
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
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .with_context(|| format!("Could not get file name for {}", path.display()))?
            .to_str()
            .with_context(|| format!("File name is not valid UTF-8 for {}", path.display()))?
            .to_owned();
        if path.is_dir() {
            let directory_tree = Tree::create(path)?;
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
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut entries: Vec<TreeEntry> = vec![];
        let dir_contents = fs::read_dir(path).with_context(|| {
            format!(
                "Unable to generate tree. Unable to read directory {}",
                path.display()
            )
        })?;
        let dir_contents: Vec<DirEntry> = dir_contents.collect::<Result<_, _>>()?;
        let rygit_path = rygit_path();
        let mut dir_contents: Vec<_> = dir_contents
            .iter()
            .filter(|d| d.path() != rygit_path)
            .collect();
        dir_contents.sort_by(|a, b| {
            a.file_name()
                .to_string_lossy()
                .cmp(&b.file_name().to_string_lossy())
        });
        for entry in dir_contents {
            entries.push(TreeEntry::create(entry.path())?);
        }
        let serialized_data = serialize(&entries);
        let hash = Hash::of(&serialized_data);

        if !hash.object_path().exists() {
            fs::create_dir_all(hash.object_path().parent().unwrap())
                .context("Unable to generate tree. Unable to create parent directory")?;
            let mut file = File::create(hash.object_path())
                .context("Unable to generate tree. Unable to create object file")?;
            let serialized_data = compress(&serialized_data)?;
            file.write_all(&serialized_data)
                .context("Unable to generate tree. Unable to write to object file")?;
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

    pub fn load(object_path: impl AsRef<Path>) -> Result<Self> {
        let mut file =
            File::open(&object_path).context("Unable to load tree. Object does not exist")?;
        let mut serialized_data = vec![];
        file.read_to_end(&mut serialized_data)
            .context("Unable to load tree. Unable to read object file")?;
        let serialized_data = decompress(&serialized_data)
            .context("Unable to load tree. Unable to decompress serialized data")?;

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
