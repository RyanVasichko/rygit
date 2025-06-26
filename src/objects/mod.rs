use strum::Display;

use crate::{
    hash::Hash,
    objects::{blob::Blob, tree::Tree},
};

pub mod blob;
pub mod tree;

#[derive(Debug, Display)]
pub enum Object {
    Blob(Blob),
    Tree(Tree),
}

impl Object {
    pub fn hash(&self) -> Hash {
        match self {
            Object::Blob(blob) => blob.hash,
            Object::Tree(tree) => tree.hash,
        }
    }
}
