use std::env;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::commands::{self};

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
            let cwd = env::current_dir().context(
                "Unable to initialize repository. Unable to determine current directory",
            )?;
            commands::init::run(cwd)?;
        }
        Commands::Commit { message } => {
            // TODO: Ensure the current directory is a repo
            commands::commit::run(message)?;
        }
        Commands::Log => commands::log::run()?,
    }

    Ok(())
}
