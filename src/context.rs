use crate::{
    Args, Commands,
    daemon::{AppState, Daemon},
    info::InfoGatherer,
};
use chrono::Utc;

use crate::{
    config::{Configuration, SOCKET_NAME, load_config},
    scheduler::{EventSource, TriggerSource},
};

pub struct Context {
    pub config: Configuration,
    pub config_path: String,
}

impl Context {
    pub fn create_from_config(config: Configuration, config_path: String) -> Self {
        Self {
            config,
            config_path,
        }
    }

    fn create_execution_state(&self, args: Args) -> crate::error::Result<AppState> {
        match args.command {
            Some(c) => match c {
                Commands::Start => self.create_daemon(),
                Commands::PrintDefaultConfig => self.create_default_config(),
            },
            None => self.gather_info(),
        }
    }
    fn create_default_config(&self) -> crate::error::Result<AppState> {
        Ok(AppState::DefaultConfig)
    }

    fn create_daemon(&self) -> crate::error::Result<AppState> {
        Ok(AppState::Daemon(Daemon::create(self)?))
    }

    pub fn run(self, args: Args) -> Result<(), Box<dyn std::error::Error + 'static>> {
        let state = self.create_execution_state(args)?;
        state.run(self)
    }

    fn gather_info(&self) -> crate::error::Result<AppState> {
        let ts = TriggerSource::from_config(&self.config)?;
        let next_event_at = ts.next_event_at(Utc::now());
        Ok(AppState::Info(InfoGatherer::new(next_event_at)))
    }
}
