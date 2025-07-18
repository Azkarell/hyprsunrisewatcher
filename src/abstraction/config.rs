use std::{fs::canonicalize, path::PathBuf, str::FromStr};

use chrono::NaiveTime;
use figment::{
    Figment,
    providers::{Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};

use super::scheduler::ActionTrigger;

#[derive(Serialize, Deserialize, Debug)]
pub struct Configuration {
    pub enabled: bool,
    pub manual: Option<ManualConfig>,
    pub automatic: Option<AutomaticConfig>,
    pub pipe: PathBuf,
    pub actions: Actions,
    pub hot_reload: bool,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            enabled: true,
            manual: Some(ManualConfig {
                time_stamps: vec![],
            }),
            automatic: None,
            actions: Actions::default(),
            pipe: PathBuf::from_str("/tmp/hyprsunrisewatcher.pipe")
                .expect("failed to specify base pipe path"),
            hot_reload: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManualTimeStamp {
    pub trigger_time: NaiveTime,
    pub action: ActionTrigger,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ManualConfig {
    pub time_stamps: Vec<ManualTimeStamp>,
}

#[derive(Serialize, Deserialize, Debug)]
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
    println!("trying to load {path}");
    let pb = shellexpand::full(&path)?;

    let figment = Figment::new()
        .merge(Serialized::defaults(Configuration::default()))
        .merge(Toml::file(&*pb));

    let config = figment.extract()?;
    println!("loaded: {config:?}");
    Ok(config)
}
