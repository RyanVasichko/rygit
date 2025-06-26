use std::io::Write;

use anyhow::Result;
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use std::io::Read;

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
