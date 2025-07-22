use clap::{Parser, Subcommand};
use config::Configuration;
use context::Context;

pub mod actions;
pub mod config;
pub mod context;
pub mod daemon;
pub mod error;
pub mod info;
pub mod scheduler;
pub mod state;

#[derive(Parser, Clone)]
#[command(version, about)]
pub struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
    #[arg(short,long, default_value = Configuration::DEFAULT_PATH)]
    config: String,
}

#[derive(Subcommand, Clone, PartialEq, Debug)]
pub enum Commands {
    Start,
    PrintDefaultConfig,
}

fn main() -> crate::error::Result<()> {
    let args = Args::parse();
    let shell_expaned = shellexpand::full(&args.config)?;
    let config = Configuration::load(&shell_expaned)?;
    let context = Context::create_from_config(config, shell_expaned.into_owned());
    context.run(args)
}

#[cfg(test)]
mod test {}
