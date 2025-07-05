use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::{
    compression::{compress, decompress},
    hash::Hash,
};

// blob format:
// <type> <size>\0<content>
#[derive(Debug, PartialEq, Eq)]
pub struct Blob {
    hash: Hash,
}

impl Blob {
    pub fn create(_path: impl AsRef<Path>) -> Result<Self> {
        let file_path = _path.as_ref();
        let serialized_data = serialize(file_path).with_context(|| {
            format!(
                "Unable to create blob contents for file {}",
                file_path.display()
            )
        })?;
        let hash = Hash::of(&serialized_data);
        let serialized_data = compress(&serialized_data)?;
        let object_path = hash.object_path();
        if !object_path.try_exists().unwrap() {
            fs::create_dir_all(object_path.parent().unwrap())
                .context("Unable to generate blob. Unable to create parent directory")?;
            let mut file = File::create(&object_path)
                .context("Unable to generate blob. Unable to create object file")?;
            file.write_all(&serialized_data)
                .context("Unable to generate blob. Unable to write object file")?;
        }

        Ok(Self { hash })
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

    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    pub fn load(object_path: PathBuf) -> Result<Self> {
        let hash = Hash::from_object_path(&object_path)?;
        let blob = Self { hash };

        Ok(blob)
    }
}
fn serialize(file_path: &Path) -> Result<Vec<u8>> {
    let file_contents = fs::read(file_path)
        .with_context(|| format!("Unable to read file {}", file_path.display()))?;
    let file_length = file_contents.len();
    let header = format!("blob {file_length}\0");

    let mut blob = Vec::with_capacity(header.len() + file_length);
    blob.extend_from_slice(header.as_bytes());
    blob.extend_from_slice(&file_contents);

    Ok(blob)
}
