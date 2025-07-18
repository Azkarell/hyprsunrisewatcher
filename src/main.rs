use abstraction::{
    actions::Action,
    config::{Configuration, SOCKET_NAME, load_config},
    daemon::Context,
};
use clap::{Parser, Subcommand};
use interprocess::local_socket::{GenericNamespaced, Stream};

pub mod abstraction;
pub mod error;
pub mod platform;

#[derive(Parser, Clone)]
#[command(version, about)]
pub struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Clone, PartialEq, Debug)]
pub enum Commands {
    Disable,
    Enable,
    Toggle,
    Stop,
    Info,
}
impl Commands {
    fn to_action(&self) -> Action {
        match self {
            Commands::Disable => Action::Disable,
            Commands::Enable => Action::Enable,
            Commands::Toggle => Action::Toggle,
            Commands::Stop => Action::Stop,
            Commands::Info => Action::Nothing,
        }
    }
}

fn main() -> crate::error::Result<()> {
    let context = init()?;
    let args = Args::parse();
    context.run(args)
}

fn init() -> crate::error::Result<Context> {
    let config = load_config(Configuration::default().config_path.display().to_string())?;
    Ok(Context::create_from_config(config))
}

#[cfg(test)]
mod test {}
