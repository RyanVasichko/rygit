use std::{env, fs};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::{
    commands::init,
    objects::{blob::Blob, tree::Tree},
};

#[derive(Parser)]
#[command(name = "rygit")]
#[command(about = "Ryan's git clone", long_about = None)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    Init { name: String },
    Fake,
}

pub(crate) fn run(cli: Cli) -> Result<()> {
    match &cli.command {
        Commands::Init { name } => {
            init::run(
                name,
                env::current_dir().context("Unable to determine the current directory")?,
            )?;
        }
        Commands::Fake => {
            // Temporary code to get the compiler to stop warning about unused code
            let current_dir = std::env::current_dir()?;
            let dir = fs::read_dir(&current_dir)?;
            for entry in dir {
                let entry = entry?;
                if entry.path().is_dir() {
                    let tree = Tree::new(entry.path().as_path())?;
                    println!("{}", tree.entries.len());
                } else if entry.path().is_file() {
                    let blob = Blob::new(entry.path().as_path())?;
                    blob.write(current_dir.as_path())?;
                }
            }
        }
    }

    Ok(())
}
