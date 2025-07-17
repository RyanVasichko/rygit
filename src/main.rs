use clap::Parser;

use crate::cli::Cli;

pub mod cli;
pub mod commands;
pub mod compression;
pub mod hash;
pub mod index;
pub mod objects;
pub mod paths;
#[cfg(test)]
pub mod test_utils;

fn main() {
    let cli = Cli::parse();
    let result = cli::run(cli);
    match result {
        Ok(_) => (),
        Err(err) => {
            for cause in err.chain() {
                eprintln!("{cause}")
            }
        }
    }
}
