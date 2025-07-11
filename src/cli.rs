use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{
    commands::{self},
    paths::repository_root_path,
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
}

pub fn run(cli: Cli) -> Result<()> {
    match &cli.command {
        Commands::Init => {
            commands::init::run(repository_root_path())?;
        }
        Commands::Commit { message } => {
            // TODO: Ensure the current directory is a repo
            commands::commit::run(message)?;
        }
        Commands::Log => commands::log::run()?,
    }

    Ok(())
}
