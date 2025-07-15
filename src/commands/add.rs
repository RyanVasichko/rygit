use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::{index::Index, paths::repository_root_path};

pub fn run(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let repository_path = repository_root_path();
    if !path.starts_with(repository_path) {
        bail!("Cannot add {}, not part of this repository", path.display())
    }
    let mut index = Index::load()
        .with_context(|| format!("Unable to add {}. Unable to generate index", path.display()))?;
    index.add(path)
}
