use std::{fs::File, io::Read};

use anyhow::{Context, Result};
use chrono::{DateTime, FixedOffset};

use crate::{hash::Hash, objects::commit::Commit, paths::head_ref_path};

pub fn run() -> Result<()> {
    let mut head_commit_file =
        File::open(head_ref_path()).context("Unable to generate log. Unable to open head ref")?;
    let mut head_commit_hash = String::new();
    head_commit_file
        .read_to_string(&mut head_commit_hash)
        .context("Unable to generate log. Unable to read head commit hash")?;
    let head_commit_hash = head_commit_hash.trim();
    let head_commit_hash = Hash::from_hex(head_commit_hash)
        .context("Unable to generate log. head commit hash is not a valid hash")?;
    let head_commit = Commit::load(&head_commit_hash)
        .context("Unable to generate log. Unable to load head commit")?;

    let mut log_contents = String::new();
    let mut commit = Some(head_commit);
    while let Some(c) = commit {
        let commit_log = commit_log(&c);
        log_contents.push_str(&commit_log);

        let parents = c.parents()?;
        commit = if !parents.is_empty() {
            Some(parents.into_iter().next().unwrap())
        } else {
            None
        };
    }

    Ok(())
}

fn commit_log(commit: &Commit) -> String {
    let mut log = String::new();
    log.push_str(&format!("commit {}", commit.hash().to_hex()));
    log.push_str(&format!(
        "Author: {} <{}>",
        commit.author().name(),
        commit.author().email()
    ));
    log.push_str(&format!(
        "Date: {}",
        format_commit_date(commit.author().timestamp())
    ));

    log
}

fn format_commit_date(timestamp: &DateTime<FixedOffset>) -> String {
    timestamp.format("%a %b %e %T %Y %z").to_string()
}
