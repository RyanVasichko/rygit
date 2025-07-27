use anyhow::{Ok, Result};

use crate::branch::Branch;

pub fn list() -> Result<()> {
    let current_branch = Branch::current()?;
    let branches = Branch::list()?;
    let branches = branches
        .iter()
        .filter(|b| b.name() != current_branch.name());

    println!("* {}", current_branch.name());
    for branch in branches {
        println!("  {}", branch.name());
    }

    Ok(())
}
