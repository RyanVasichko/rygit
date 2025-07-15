use anyhow::Result;

use crate::{
    index::Index,
    objects::{commit::Commit, signature::Signature},
};

pub fn run(message: impl Into<String>) -> Result<()> {
    let author = Signature::new("Larry Sellers", "lsellers@test.com");
    let index = Index::load()?;
    Commit::create(&index, message, author.clone(), author)?;

    Ok(())
}
