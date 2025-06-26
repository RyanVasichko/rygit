use clap::Parser;

use crate::cli::Cli;

mod cli;
mod commands;
mod hash;
mod objects;
mod utils;

fn main() {
    let cli = Cli::parse();
    let result = cli::run(cli);
    match result {
        Ok(_) => (),
        Err(err) => {
            for cause in err.chain() {
                eprintln!("{}", cause)
            }
        }
    }
}
