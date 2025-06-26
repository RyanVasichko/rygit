use anyhow::{Context, Result};
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash([u8; 20]);

impl Hash {
    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(hex: &str) -> Result<Self> {
        let bytes = hex::decode(hex).with_context(|| format!("Invalid hex string: {}", hex))?;
        if bytes.len() != 20 {
            return Err(anyhow::anyhow!("Hash must be exactly 20 bytes"));
        }
        let mut hash_bytes = [0u8; 20];
        hash_bytes.copy_from_slice(&bytes);
        Ok(Hash(hash_bytes))
    }

    pub fn of(data: &[u8]) -> Self {
        let mut hasher = Sha1::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut hash_bytes = [0u8; 20];
        hash_bytes.copy_from_slice(&result);
        Hash(hash_bytes)
    }

    pub fn to_object_path(&self) -> PathBuf {
        let hash_hex = self.to_hex();
        Path::new(&hash_hex[0..2]).join(&hash_hex[2..])
    }
}

impl From<[u8; 20]> for Hash {
    fn from(bytes: [u8; 20]) -> Self {
        Hash(bytes)
    }
}

impl Into<[u8; 20]> for Hash {
    fn into(self) -> [u8; 20] {
        self.0
    }
}

impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}
