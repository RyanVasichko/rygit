use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha1::{Digest, Sha1};

use crate::paths::objects_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash([u8; 20]);

impl Hash {
    pub fn new(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(hex: &str) -> Result<Self> {
        let bytes = hex::decode(hex).with_context(|| format!("Invalid hex string: {hex}"))?;
        if bytes.len() != 20 {
            return Err(anyhow::anyhow!("Hash must be exactly 20 bytes"));
        }
        let mut hash_bytes = [0u8; 20];
        hash_bytes.copy_from_slice(&bytes);
        Ok(Hash(hash_bytes))
    }

    pub fn from_object_path(object_path: impl AsRef<Path>) -> Result<Self> {
        let object_path = object_path.as_ref();
        let parent = object_path.parent().context(
            "Unable to determine object path. Unable to determine blob parent directory",
        )?;
        let parent_file_name = parent
            .file_name()
            .context(
                "Unable to determine object path. Unable to determine blob parent directory name",
            )?
            .to_str()
            .context("Unable to determine object path. Unable to convert path to string")?;
        let file_name = object_path
            .file_name()
            .context("Unable to determine object path. Unable to determine blob file name")?
            .to_str()
            .context("Unable to determine object path. Unable to convert path to string")?;
        let hex = format!("{parent_file_name}{file_name}");
        let hash = Hash::from_hex(&hex)
            .context("Unable to determine object path. Unable to generate hash from blob path")?;

        Ok(hash)
    }

    pub fn of(data: &[u8]) -> Self {
        let mut hasher = Sha1::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut hash_bytes = [0u8; 20];
        hash_bytes.copy_from_slice(&result);
        Self(hash_bytes)
    }

    pub fn object_path(&self) -> PathBuf {
        let hash_hex = self.to_hex();
        objects_path().join(&hash_hex[0..2]).join(&hash_hex[2..])
    }
}

impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}
