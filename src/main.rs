use std::path::PathBuf;

use abstraction::{
    actions::Action,
    config::{Configuration, SOCKET_NAME, load_config},
    daemon::Context,
    scheduler::ActionTrigger,
};
use chrono::{DateTime, Local};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

pub mod abstraction;
pub mod error;
pub mod platform;

#[derive(Parser, Clone)]
#[command(version, about, arg_required_else_help = true)]
pub struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}
#[derive(Serialize)]
pub struct EventInfo {
    at: DateTime<Local>,
    triggers: ActionTrigger,
}
#[derive(Serialize)]
pub struct Info<'a> {
    daemon_running: bool,
    next_event: Option<EventInfo>,
    configuration: &'a Configuration,
}

#[derive(Subcommand, Clone, PartialEq, Debug)]
pub enum Commands {
    Disable,
    Enable,
    Toggle,
    Stop,
    Start,
    Info,
    GenerateDefaultConfig {
        #[arg(
            short,
            long,
            default_value = "~/.config/hyprsunrisewatcher/config.toml"
        )]
        path: PathBuf,
    },
}

impl Commands {
    fn to_action(&self) -> Action {
        match self {
            Commands::Disable => Action::Disable,
            Commands::Enable => Action::Enable,
            Commands::Toggle => Action::Toggle,
            Commands::Stop => Action::Stop,
            Commands::Info => Action::Nothing,
            Commands::GenerateDefaultConfig { path } => {
                Action::GenerateDefaultConfig { path: path.clone() }
            }
            Commands::Start => Action::Nothing,
        }
    }
}

fn main() -> crate::error::Result<()> {
    let config = Configuration::load();
    let context = Context::create_from_config(config);
    let args = Args::parse();
    context.run(args)
}

fn init() -> crate::error::Result<Context> {
    let config = load_config(Configuration::default().config_path.display().to_string())?;
    Ok(Context::create_from_config(config))
}

#[cfg(test)]
mod test {}
