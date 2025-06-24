use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands::init;

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
}

pub(crate) fn run(cli: Cli) -> Result<()> {
    match &cli.command {
        Commands::Init { name } => {
            init::run(name);
        }
    }

    Ok(())
}
