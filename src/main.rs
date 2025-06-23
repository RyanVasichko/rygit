use clap::{Parser, Subcommand};

/// A simple CLI app with multiple commands
#[derive(Parser)]
#[command(name = "rygit")]
#[command(about = "Ryan's git clone", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init { name: String },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init { name } => {
            println!("Repository \"{}\" initialized!", name);
        }
    }
}
