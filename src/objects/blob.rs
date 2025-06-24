use anyhow::{Context, Result};
use flate2::{Compression, write::ZlibEncoder};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use crate::utils::{self, compress};

pub struct Blob {
    pub path: PathBuf,
    pub hash: [u8; 20],
}

impl Blob {
    pub fn new(file_path: &Path, rygit_objects_path: &Path) -> Result<Self> {
        let blob = create_blob_contents_from_file(file_path).with_context(|| {
            format!(
                "Unable to create blob contents for file {}",
                file_path.display()
            )
        })?;

        let hash = utils::hash(&blob);
        let hex_hash = hex::encode(hash);
        let blob_folder = &hex_hash[0..2];
        let blob_file_name = &hex_hash[2..];
        let blob_file_path = rygit_objects_path.join(blob_folder).join(blob_file_name);

        if blob_file_path.exists() {
            return Ok(Blob {
                path: blob_file_path,
                hash,
            });
        }

        if let Some(parent) = blob_file_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Unable to create parent directories {}", parent.display())
            })?;
        }

        let mut blob_file = File::create(&blob_file_path)
            .with_context(|| format!("Unable to create blob file {}", blob_file_path.display()))?;
        let blob = compress(&blob)
            .with_context(|| format!("Unable to compress blob {}", blob_file_path.display()))?;

        blob_file
            .write_all(&blob)
            .with_context(|| format!("Unable to write blob file {}", blob_file_path.display()))?;

        Ok(Blob {
            path: blob_file_path,
            hash,
        })
    }
}

fn create_blob_contents_from_file(file_path: &Path) -> Result<Vec<u8>> {
    let file_contents = fs::read(file_path)
        .with_context(|| format!("Unable to read file {}", file_path.display()))?;
    let file_length = file_contents.len();
    let header = format!("blob {}\0", file_length);

    let mut blob = Vec::with_capacity(header.len() + file_length);
    blob.extend_from_slice(header.as_bytes());
    blob.extend_from_slice(&file_contents);

    Ok(blob)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Seek, SeekFrom, Write};

    use anyhow::Result;
    use tempfile::{NamedTempFile, TempDir};

    use super::*;

    #[test]
    fn test_new() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        let file_contents = "Hello world\n\n";
        write!(file, "{}", file_contents)?;
        let folder = TempDir::new()?;

        let blob = Blob::new(file.path(), folder.path())?;
        let mut blob_file = File::open(blob.path)?;
        let mut blob_file_contents: Vec<u8> = vec![];
        blob_file.seek(SeekFrom::Start(0))?;
        blob_file.read_to_end(&mut blob_file_contents)?;

        let expected = format!("blob 13\0{}", file_contents).into_bytes();
        let expected = compress(&expected)?;
        assert_eq!(expected, blob_file_contents);

        let expected_hash = utils::hash(&format!("blob 13\0{}", file_contents).into_bytes());
        assert_eq!(blob.hash, expected_hash);

        Ok(())
    }
}
