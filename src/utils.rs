use std::io::Write;

use anyhow::Result;
use flate2::{Compression, write::ZlibEncoder};
use sha1::{Digest, Sha1};

pub fn hash(contents: &[u8]) -> [u8; 20] {
    let mut hasher = Sha1::new();
    hasher.update(contents);

    hasher.finalize().into()
}

pub fn compress(contents: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(contents)?;
    let compressed = encoder.finish()?;

    Ok(compressed)
}
