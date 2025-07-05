use anyhow::Result;

use crate::objects::{commit::Commit, signature::Signature};

pub fn run(message: impl Into<String>) -> Result<()> {
    let author = Signature::new("Larry Sellers", "lsellers@test.com");
    Commit::create(message, author.clone(), author)?;

    Ok(())
}
