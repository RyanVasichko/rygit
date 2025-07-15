use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
};

use anyhow::{Context, Result, bail};

use crate::{
    compression::{compress, decompress},
    hash::Hash,
    index::Index,
    objects::{
        signature::{Signature, SignatureKind},
        tree::Tree,
    },
    paths::head_ref_path,
};

// commit format:
// commit <content length>\0<commit content>
// content format:
// tree <tree_hash>
// parent <parent_hash>
// author <author_name> <<author_email>> <timestamp>
// committer <committer_name> <<committer_email>> <timestamp>
//
// <commit message>
pub struct Commit {
    _message: String,
    tree_hash: Hash,
    hash: Hash,
    parent_hashes: Vec<Hash>,
    author: Signature,
    _committer: Signature,
}

impl Commit {
    pub fn create(
        index: &Index,
        message: impl Into<String>,
        author: Signature,
        committer: Signature,
    ) -> Result<Self> {
        let mut parent_hashes: Vec<Hash> = vec![];
        let mut head_ref_contents = String::new();
        File::open(head_ref_path())
            .context("Unable to create commit. Unable to open head ref")?
            .read_to_string(&mut head_ref_contents)
            .context("Unable to create commit. Unable to read head ref")?;
        if !head_ref_contents.is_empty() {
            let head_ref_hash = Hash::from_hex(&head_ref_contents)
                .context("Unable to create commit. head ref is not a valid hash")?;
            parent_hashes.push(head_ref_hash);
        }
        let tree = Tree::create(index)?;
        let message: String = message.into();

        let serialized_data =
            Commit::serialize(&author, &committer, &parent_hashes, &tree, &message);

        let hash = Hash::of(&serialized_data);
        let serialized_data = compress(&serialized_data)
            .context("Unable to create commit. Unable to compress serialized data")?;
        let object_path = hash.object_path();
        if let Some(parent) = object_path.parent() {
            fs::create_dir_all(parent)
                .context("Unable to create commit. Unable to create object file")?;
        }
        let mut commit_object_file = File::create(hash.object_path())
            .context("Unable to create commit. Unable to create object file")?;
        commit_object_file
            .write_all(&serialized_data)
            .context("Unable to create commit. Unable to write to object file")?;

        let mut head_ref_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(head_ref_path())
            .context("Unable to create commit. Unable to open head ref")?;
        head_ref_file
            .write_all(hash.to_hex().as_bytes())
            .context("Unable to create commit. Unable to open head ref")?;

        let commit = Self {
            _message: message,
            tree_hash: *tree.hash(),
            hash,
            parent_hashes,
            author,
            _committer: committer,
        };
        Ok(commit)
    }

    pub fn load(hash: &Hash) -> Result<Self> {
        let commit_path = hash.object_path();
        let contents =
            fs::read(commit_path).context("Unable to load commit. Unable to read object file")?;
        let contents =
            decompress(&contents).context("Unable to load commit. Unable to decompress object")?;
        Commit::deserialize(contents)
    }

    fn deserialize(serialized_data: Vec<u8>) -> Result<Self> {
        let serialized_data = String::from_utf8(serialized_data)
            .context("Unable to parse commit file. Contents are not valid UTF-8")?;

        let invalid_format_message = "Unable to parse commit file. Invalid format";
        let mut parts = serialized_data.split('\0');
        let header = parts.next().context(invalid_format_message)?;
        let body = parts.next().context(invalid_format_message)?;

        // Ensure header is in correct format
        let mut header_parts = header.split(" ");
        let label = header_parts.next().context(invalid_format_message)?;
        if label != "commit" {
            bail!(invalid_format_message)
        }
        header_parts.next().context(invalid_format_message)?;

        // Parse tree hash
        let mut body_lines = body.lines().peekable();
        let tree_line = body_lines.next().context(invalid_format_message)?;
        let tree_hash = {
            let mut parts = tree_line.split(" ");
            let label = parts.next().context(invalid_format_message)?;
            if label != "tree" {
                bail!(invalid_format_message)
            }
            let hash = parts.next().context(invalid_format_message)?;
            Hash::from_hex(hash).context(invalid_format_message)?
        };

        // Parse parent hashes
        let mut parent_hashes = vec![];
        let mut peek = body_lines.peek().context(invalid_format_message)?;

        while peek.starts_with("parent") {
            let parent_line = body_lines.next().context(invalid_format_message)?;
            let mut parts = parent_line.split(" ");
            let label = parts.next().context(invalid_format_message)?;
            if label != "parent" {
                bail!(invalid_format_message)
            }
            let hash = parts.next().context(invalid_format_message)?;
            let hash = Hash::from_hex(hash).context(invalid_format_message)?;
            parent_hashes.push(hash);
            peek = body_lines.peek().context(invalid_format_message)?;
        }

        // Parse signatures
        let author_line = body_lines.next().context(invalid_format_message)?;
        let author = Signature::deserialize(author_line).context(invalid_format_message)?;
        let committer_line = body_lines.next().context(invalid_format_message)?;
        let committer = Signature::deserialize(committer_line).context(invalid_format_message)?;

        // Skip the empty line
        body_lines.next().context(invalid_format_message)?;

        let message = body_lines.collect::<Vec<_>>().join("\n");

        let hash = Hash::of(serialized_data.as_bytes());

        Ok(Self {
            hash,
            tree_hash,
            parent_hashes,
            author,
            _committer: committer,
            _message: message,
        })
    }

    fn serialize(
        author: &Signature,
        committer: &Signature,
        parent_hashes: &[Hash],
        tree: &Tree,
        message: impl Into<String>,
    ) -> Vec<u8> {
        let mut serialized_body = vec![format!("tree {}", tree.hash().to_hex())];
        for parent_hash in parent_hashes.iter() {
            serialized_body.push(format!("parent {}", parent_hash.to_hex()));
        }
        serialized_body.push(author.serialize_as(SignatureKind::Author));
        serialized_body.push(committer.serialize_as(SignatureKind::Committer));
        serialized_body.push(String::new());
        serialized_body.push(message.into());
        let serialized_body = serialized_body.join("\n");
        let serialized_body_len = serialized_body.len();

        format!("commit {serialized_body_len}\0{serialized_body}",)
            .as_bytes()
            .to_vec()
    }

    pub fn tree(&self) -> Result<Tree> {
        Tree::load(self.tree_hash.object_path())
    }

    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    pub fn author(&self) -> &Signature {
        &self.author
    }

    pub fn parents(&self) -> Result<Vec<Commit>> {
        self.parent_hashes.iter().map(Commit::load).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        fs::{self, File},
        io::{Read, Write},
        path::Path,
    };

    use anyhow::{Ok, Result};
    use tempfile::TempDir;

    use crate::{
        commands::init,
        objects::{Object, tree::TreeEntry},
        paths::head_ref_path,
    };

    use super::*;

    fn assert_tree_entry_blob(entry: &TreeEntry, name: &str, expected_body: &[u8]) {
        assert_eq!(name, entry.name());
        if let Object::Blob(blob) = entry.object() {
            assert_eq!(expected_body, &blob.body().unwrap());
        } else {
            panic!("Expected blob")
        }
    }

    fn create_test_file(path: impl AsRef<Path>, content: &[u8]) -> Result<()> {
        File::create(path.as_ref())?.write_all(content)?;
        Ok(())
    }

    #[test]
    fn test_create_commit() -> Result<()> {
        let repository = TempDir::new()?;
        let repository_path = repository.path().canonicalize().unwrap();
        env::set_current_dir(&repository_path)?;

        init::run(&repository_path)?;

        create_test_file(repository_path.join("a.txt"), b"a")?;
        create_test_file(repository_path.join("b.txt"), b"b")?;

        let subdir_path = repository_path.join("subdir");
        fs::create_dir(&subdir_path)?;

        create_test_file(subdir_path.join("c.txt"), b"c")?;

        let author = Signature::new("Larry Sellers", "l.sellers@example.com");
        let committer = Signature::new("Donny Kerabatsos", "d.kerabatsos@example.com");

        let mut index = Index::load()?;
        index.add(&repository_path)?;
        let first_commit = Commit::create(&index, "Initial commit", author, committer)?;
        let first_commit = Commit::load(first_commit.hash())?;

        let tree = first_commit.tree()?;
        assert_eq!(3, tree.entries().len());

        let mut entries_iter = tree.entries().iter();
        assert_tree_entry_blob(entries_iter.next().unwrap(), "a.txt", b"a");
        assert_tree_entry_blob(entries_iter.next().unwrap(), "b.txt", b"b");

        let entry = entries_iter.next().unwrap();
        if let Object::Tree(tree) = entry.object() {
            assert_eq!(entry.name(), "subdir");
            assert_eq!(1, tree.entries().len());
            let entry = tree.entries().first().unwrap();
            assert_tree_entry_blob(entry, "c.txt", b"c");
        } else {
            bail!(
                "Expected entry to be an Object::Tree, but got {:?}",
                entry.object()
            );
        }

        let mut head_ref_file = File::open(head_ref_path())?;
        let mut head_ref_commit = String::new();
        head_ref_file.read_to_string(&mut head_ref_commit)?;
        let head_ref_hash = Hash::from_hex(&head_ref_commit)?;
        assert_eq!(first_commit.hash, head_ref_hash);

        assert_eq!("Initial commit", first_commit._message);

        assert_eq!("Larry Sellers", first_commit.author.name());
        assert_eq!("l.sellers@example.com", first_commit.author.email());

        assert_eq!("Donny Kerabatsos", first_commit._committer.name());
        assert_eq!("d.kerabatsos@example.com", first_commit._committer.email());

        create_test_file(repository_path.join("t.txt"), b"t")?;
        let author = Signature::new("Leroy Jenkins", "l.jenkins@example.com");
        let committer = Signature::new("Larry Sellers", "l.sellers@example.com");
        index.add(&repository_path)?;
        let second_commit = Commit::create(&index, "Second commit", author, committer)?;
        let second_commit = Commit::load(second_commit.hash())?;

        assert_eq!(1, second_commit.parent_hashes.len());
        assert_eq!(
            first_commit.hash(),
            second_commit.parent_hashes.first().unwrap()
        );

        let second_commit_tree = second_commit.tree()?;
        let entries = second_commit_tree.entries();
        assert_eq!(4, entries.len());
        let mut entries = entries.iter();
        assert_tree_entry_blob(entries.next().unwrap(), "a.txt", b"a");
        assert_tree_entry_blob(entries.next().unwrap(), "b.txt", b"b");
        let entry = entries.next().unwrap();
        if let Object::Tree(tree) = entry.object() {
            assert_eq!(entry.name(), "subdir");
            assert_eq!(1, tree.entries().len());
            let entry = tree.entries().first().unwrap();
            assert_tree_entry_blob(entry, "c.txt", b"c");
        } else {
            bail!(
                "Expected entry to be an Object::Tree, but got {:?}",
                entry.object()
            );
        }
        assert_tree_entry_blob(entries.next().unwrap(), "t.txt", b"t");

        Ok(())
    }
}
