use std::{env, path::Path};

use anyhow::{Context, Ok, Result, bail};
use clap::{Parser, Subcommand};

use crate::{
    branch::Branch,
    commands::{self},
    paths::discover_repository_root_from,
};

#[derive(Parser)]
#[command(name = "rygit")]
#[command(about = "Ryan's git clone", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Init,
    Commit {
        #[clap(short, long)]
        message: String,
    },
    Log,
    Add {
        #[clap()]
        path: String,
    },
    Status,
    Branch {
        name: Option<String>,
    },
    Switch {
        name: String,
        #[clap(short, long)]
        create: bool,
    },
}

pub fn run(cli: Cli) -> Result<()> {
    let current_dir = env::current_dir().context("Unable to determine current directory")?;

    match cli.command {
        Commands::Init => {}
        _ => ensure_rygit_repository(&current_dir)?,
    }
    match &cli.command {
        Commands::Init => commands::init::run(current_dir)?,
        Commands::Commit { message } => commands::commit::run(message)?,
        Commands::Log => commands::log::run()?,
        Commands::Add { path } => {
            let mut path = Path::new(&path).to_path_buf();
            if path.is_relative() {
                let current_dir = env::current_dir()
                    .context("Unable to add. Unable to determine current directory")?;
                path = current_dir.join(path);
            }
            if !path.exists() {
                bail!("Cannot add \"{}\", not a valid path", path.display());
            }
            commands::add::run(path)?;
        }
        Commands::Status => commands::status::run()?,
        Commands::Branch { name } => {
            if let Some(name) = name {
                Branch::create(name)?;
            } else {
                commands::branch::list()?;
            }
        }
        Commands::Switch { name, create } => {
            if *create {
                Branch::create(name)?;
            }

            Branch::switch(name)?;
        }
    };

    Ok(())
}

fn ensure_rygit_repository(path: impl AsRef<Path>) -> Result<()> {
    let repo_root = discover_repository_root_from(path);
    if repo_root.is_err() {
        bail!("Not inside a repository")
    }

    Ok(())
}
