use std::fs::File;

use clap::{Command, Parser};
use clio::{Input, Output};

pub mod abstraction;
pub mod error;
pub mod platform;

#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[arg(short, long, default_value = "/tmp/hyprsunrisewatcher.pipe")]
    pipe: String,
    #[clap(value_parser, default_value = "-")]
    input: Input,
    #[clap(value_parser, default_value = "-")]
    output: Output,
}

#[derive(clap::clap_derive::Subcommand)]
pub enum Commands {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut p: File;
    if let Ok(pipe) = platform::make_pipe(&args.pipe) {
        p = pipe;
    } else {
        p = File::options().write(true).open(&args.pipe)?;
    }

    Ok(())
}

#[cfg(test)]
mod test {}
