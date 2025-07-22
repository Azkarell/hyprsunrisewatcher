use crate::{config::Configuration, context::Context, daemon::Daemon, info::InfoGatherer};

pub enum AppState {
    Daemon(Daemon),
    Info(InfoGatherer),
    DefaultConfig,
}

impl AppState {
    pub fn run(self, context: Context) -> crate::error::Result<()> {
        match self {
            AppState::Daemon(daemon) => daemon.run(context)?,
            AppState::Info(info) => info.print(context)?,
            AppState::DefaultConfig => {
                println!(
                    "{}",
                    toml_edit::ser::to_string_pretty(&Configuration::default())?
                )
            }
        }

        Ok(())
    }
}
