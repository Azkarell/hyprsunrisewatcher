use std::fs::File;

use abstraction::{
    actions::Action,
    config::{Configuration, load_config},
    daemon::{check_daemon_running, start_daemon},
};
use clap::{Parser, Subcommand};

pub mod abstraction;
pub mod error;
pub mod platform;

#[derive(Parser, Clone)]
#[command(version, about)]
pub struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
    #[arg(short, long, default_value = "~/.config/hyprsunrisewatcher/config")]
    config: String,
}

#[derive(Subcommand, Clone, PartialEq, Debug)]
pub enum Commands {
    Disable,
    Enable,
    Toggle,
    Stop,
}

fn main() -> crate::error::Result<()> {
    let (config, args) = init()?;

    let daemon_is_running = check_daemon_running(&config.pipe)?;
    if daemon_is_running && args.command.is_none() {
        return Err(crate::error::Error::DaemonAlreadyRunning.into());
    } else if !daemon_is_running {
        return start_daemon(config, args);
    }
    run_as_cmd(args, config)?;

    Ok(())
}

fn run_as_cmd(args: Args, config: Configuration) -> crate::error::Result<()> {
    if let Some(cmd) = args.command
        && let Ok(mut p) = File::options().write(true).open(&config.pipe)
    {
        let bincode_config = bincode::config::standard();
        match cmd {
            Commands::Disable => {
                bincode::encode_into_std_write(Action::Disable, &mut p, bincode_config)?
            }
            Commands::Enable => {
                bincode::encode_into_std_write(Action::Enable, &mut p, bincode_config)?
            }
            Commands::Toggle => {
                bincode::encode_into_std_write(Action::Toggle, &mut p, bincode_config)?
            }
            Commands::Stop => bincode::encode_into_std_write(Action::Stop, &mut p, bincode_config)?,
        };
    }
    Ok(())
}

fn init() -> crate::error::Result<(Configuration, Args)> {
    let args = Args::parse();
    let config = load_config(args.config.clone())?;
    Ok((config, args))
}

#[cfg(test)]
mod test {}
