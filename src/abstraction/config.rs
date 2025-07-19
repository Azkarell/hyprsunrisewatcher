use std::{fmt::Display, fs::canonicalize, path::PathBuf, str::FromStr};

use chrono::NaiveTime;
use figment::{
    Figment,
    providers::{Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};
use toml_edit::ser::to_string_pretty;

use super::scheduler::ActionTrigger;
pub static SOCKET_NAME: &str = "hyprsunrisewatcher.sock";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Configuration {
    pub enabled: bool,
    pub manual: Option<ManualConfig>,
    pub automatic: Option<AutomaticConfig>,
    pub actions: Actions,
    pub hot_reload: bool,
}
pub static DEFAULT_PATH: &str = "~/.config/hyprsunrisewatcher/config.toml";

impl Configuration {
    pub(crate) fn load_default() -> crate::error::Result<Configuration> {
        Self::load(DEFAULT_PATH)
    }

    pub fn load(path: &str) -> crate::error::Result<Configuration> {
        let pb = shellexpand::full(path)?;

        let figment = Figment::new()
            .merge(Serialized::defaults(Configuration::default()))
            .merge(Toml::file(&*pb));

        let mut config: Configuration = figment.extract()?;

        Ok(config)
    }
}

impl Display for Configuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let serialized = to_string_pretty(self).map_err(|_| std::fmt::Error)?;
        f.write_str(&serialized)
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            config_path: "~/.config/hyprsunrisewatcher/config.toml".into(),
            enabled: true,
            manual: Some(ManualConfig {
                time_stamps: vec![],
            }),
            automatic: None,
            actions: Actions::default(),
            hot_reload: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManualTimeStamp {
    pub trigger_time: NaiveTime,
    pub action: ActionTrigger,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManualConfig {
    pub time_stamps: Vec<ManualTimeStamp>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AutomaticConfig {
    pub longitude: f64,
    pub latitude: f64,
}
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Actions {
    on_sunrise: Option<String>,
    on_sunset: Option<String>,
    on_dawn: Option<String>,
    on_dusk: Option<String>,
}

impl Actions {
    pub fn get(&self, trigger: ActionTrigger) -> Option<String> {
        match trigger {
            ActionTrigger::Sunrise => self.on_sunrise.clone(),
            ActionTrigger::Sunset => self.on_sunset.clone(),
            ActionTrigger::Dusk => self.on_dusk.clone(),
            ActionTrigger::Dawn => self.on_dawn.clone(),
        }
    }
}
pub fn load_config(path: String) -> crate::error::Result<Configuration> {
    let pb = shellexpand::full(&path)?;

    let figment = Figment::new()
        .merge(Serialized::defaults(Configuration::default()))
        .merge(Toml::file(&*pb));

    let mut config: Configuration = figment.extract()?;

    config.config_path = pb.as_ref().into();
    Ok(config)
}
