use std::{
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::Result;
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use sha1::{Digest, Sha1};
use std::io::Read;

pub fn hash(contents: &[u8]) -> [u8; 20] {
    let mut hasher = Sha1::new();
    hasher.update(contents);

    hasher.finalize().into()
}

pub fn hash_to_object_path(hash: &[u8; 20]) -> PathBuf {
    let hash_hex = hex::encode(hash);
    Path::new(&hash_hex[0..2]).join(&hash_hex[2..])
}

pub fn compress(contents: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(contents)?;
    let compressed = encoder.finish()?;

    Ok(compressed)
}

pub fn decompress(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(compressed);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;

    Ok(decompressed)
}
