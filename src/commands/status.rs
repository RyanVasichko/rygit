use anyhow::Result;

use crate::{
    branch::Branch,
    paths::repository_root_path,
    repository_status::{RepositoryStatus, StatusEntry},
};

pub fn run() -> Result<()> {
    let status = RepositoryStatus::load()?;
    let current_branch = Branch::current()?;
    println!("On branch {}", current_branch.name());

    println!("Changes to be committed:");
    for staged_change in status.staged_changes() {
        print_status_entry(staged_change)?;
    }

    println!("Changes not staged for commit:");
    for unstaged_change in status.unstaged_changes() {
        print_status_entry(unstaged_change)?;
    }

    let repository_root = repository_root_path();
    for untracked_file in status.untracked_files() {
        let relative_path = untracked_file.strip_prefix(&repository_root)?.display();
        println!("\t{relative_path}");
    }

    Ok(())
}

fn print_status_entry(status_entry: &StatusEntry) -> Result<()> {
    let repository_root = repository_root_path();
    let status_string = status_entry.status.to_string().to_lowercase();
    let relative_path = status_entry.path.strip_prefix(&repository_root)?.display();
    println!("\t{status_string}: {relative_path}");

    Ok(())
}
