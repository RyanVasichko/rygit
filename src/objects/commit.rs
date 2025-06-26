use chrono::{DateTime, FixedOffset, Local, Utc};

use crate::hash::Hash;

enum SignatureKind {
    Author,
    Committer,
}

struct Signature {
    name: String,
    email: String,
    timestamp: DateTime<FixedOffset>,
}

impl Signature {
    fn new(name: String, email: String) -> Self {
        Self {
            name,
            email,
            timestamp: Local::now().fixed_offset(),
        }
    }

    pub fn serialize_as(&self, kind: SignatureKind) -> String {
        let kind = match kind {
            SignatureKind::Author => "author",
            SignatureKind::Committer => "committer",
        };
        format!(
            "{} {} <{}> {} {}",
            kind,
            self.name,
            self.email,
            self.timestamp.timestamp(),
            format_offset(self.timestamp.offset().local_minus_utc())
        )
    }
}

struct Commit {
    hash: Hash,
    serialized_data: Vec<u8>,
    tree_hash: Hash,
    parent_hashes: Vec<Hash>,
    author: Signature,
    committer: Signature,
    message: String,
}

impl Commit {
    pub fn new(
        tree_hash: Hash,
        parent_hashes: Vec<Hash>,
        author: Signature,
        committer: Signature,
        message: String,
    ) -> Self {
        let mut serialized_body = vec![format!("tree {}", tree_hash.to_hex())];
        for parent_hash in parent_hashes.iter() {
            serialized_body.push(format!("parent {}", parent_hash.to_hex()));
        }
        serialized_body.push(author.serialize_as(SignatureKind::Author));
        serialized_body.push(committer.serialize_as(SignatureKind::Committer));
        serialized_body.push(String::new());
        serialized_body.push(message.clone());
        let serialized_body = serialized_body.join("\n");
        let serialized_data = format!(
            "commit {}\0{}",
            serialized_body.as_bytes().len(),
            serialized_body
        )
        .as_bytes()
        .to_vec();
        let hash = Hash::of(&serialized_data);

        Self {
            hash,
            serialized_data,
            tree_hash,
            parent_hashes,
            author,
            committer,
            message,
        }
    }
}

fn format_offset(offset_seconds: i32) -> String {
    let sign = if offset_seconds >= 0 { '+' } else { '-' };
    let offset_minutes = offset_seconds.abs() / 60;
    let hours = offset_minutes / 60;
    let minutes = offset_minutes % 60;
    format!("{}{:02}{:02}", sign, hours, minutes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, TimeZone};

    fn fixed_signature(name: &str, email: &str) -> Signature {
        Signature {
            name: name.to_string(),
            email: email.to_string(),
            timestamp: FixedOffset::east_opt(3600)
                .unwrap()
                .with_ymd_and_hms(2024, 1, 1, 12, 0, 0)
                .unwrap(),
        }
    }

    fn dummy_hash(byte: u8) -> Hash {
        Hash::from([byte; 20])
    }

    #[test]
    fn test_signature_serialize_as() {
        let sig = fixed_signature("Alice", "alice@example.com");
        let author = sig.serialize_as(SignatureKind::Author);
        assert!(author.starts_with("author Alice <alice@example.com> "));
        let committer = sig.serialize_as(SignatureKind::Committer);
        assert!(committer.starts_with("committer Alice <alice@example.com> "));
    }

    #[test]
    fn test_commit_new_and_serialization() {
        let tree_hash = dummy_hash(1);
        let parent_hashes = vec![dummy_hash(2), dummy_hash(3)];
        let author = fixed_signature("Alice", "alice@example.com");
        let committer = fixed_signature("Bob", "bob@example.com");
        let message = "Initial commit".to_string();
        let commit = Commit::new(
            tree_hash,
            parent_hashes.clone(),
            author,
            committer,
            message.clone(),
        );
        // Check that the commit contains the expected hashes and message
        assert_eq!(commit.tree_hash, tree_hash);
        assert_eq!(commit.parent_hashes, parent_hashes);
        assert_eq!(commit.message, message);
        // Check that serialized_data starts with the correct header
        assert!(String::from_utf8_lossy(&commit.serialized_data).starts_with("commit "));
    }
}
