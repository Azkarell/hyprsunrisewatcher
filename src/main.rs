use std::{
    fmt::Display,
    fs::File,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{Receiver, Sender},
    },
    thread::JoinHandle,
    vec,
};

use abstraction::scheduler::{self, Action, ProcessCmd, Scheduler};
use bincode::{Decode, Encode};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Utc};
use clap::{Parser, Subcommand};
use clio::{Input, Output};
use figment::{
    Figment,
    providers::{Format, Serialized, Toml},
};
use nix::unistd::sleep;
use serde::{Deserialize, Serialize};
use sunrise::Coordinates;

pub mod abstraction;
pub mod error;
pub mod platform;

#[derive(Parser, Clone)]
#[command(version, about)]
struct Args {
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

#[derive(Serialize, Deserialize, Debug)]
pub struct ManualTimeStamp {
    trigger_time: NaiveTime,
    action: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ManualConfig {
    time_stamps: Vec<ManualTimeStamp>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutomaticConfig {
    longitude: f64,
    latitude: f64,
    on_sunrise: String,
    on_sunset: String,
    on_dawn: String,
    on_dusk: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Configuration {
    enabled: bool,
    manual: Option<ManualConfig>,
    automatic: Option<AutomaticConfig>,
    pipe: PathBuf,
    hot_reload: bool,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            enabled: true,
            manual: Some(ManualConfig {
                time_stamps: vec![],
            }),
            automatic: None,
            pipe: PathBuf::from_str("/tmp/hyprsunrisewatcher.pipe")
                .expect("failed to specify base pipe path"),
            hot_reload: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, PartialEq, Eq, Clone)]
pub enum Action {
    Stop,
    Enable,
    Disable,
    Toggle,
    ReloadConfig { path: String },
    Trigger { action: String },
}

impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Stop => f.write_str("Action - Stop"),
            Action::Enable => f.write_str("Action - Enable"),
            Action::Disable => f.write_str("Action - Disable"),
            Action::Toggle => f.write_str("Action - Toggle"),
            Action::ReloadConfig { path } => f.write_str(&format!("Action - Relod - {}", path)),
            Action::Trigger { action } => f.write_str(&format!("Action - Trigger - {}", action)),
        }
    }
}

fn main() -> crate::error::Result<()> {
    let (config, args) = init()?;

    let daemon_is_running = check_daemon_running(&config.pipe)?;
    if daemon_is_running && args.command == None {
        return Err(crate::error::Error::DaemonAlreadyRunning.into());
    } else if !daemon_is_running {
        return start_daemon(config, args);
    }

    // we start handling updates according to the configuration
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

fn check_daemon_running<P: AsRef<Path>>(path: P) -> crate::error::Result<bool> {
    Ok(std::fs::exists(path)?)
}

pub struct DaemonState {
    translate_should_run: Arc<AtomicBool>,
    translate_thread: JoinHandle<()>,
    handler_thread: JoinHandle<()>,
    args: Args,
}

impl DaemonState {
    pub fn create_from_config(config: Configuration, args: Args) -> crate::error::Result<Self> {
        let (tx, tr) = std::sync::mpsc::channel();
        let is_running = Arc::new(AtomicBool::new(true));
        let pipe = platform::make_pipe(&config.pipe)?;
        let join_handle = start_translate_pipe_events(tx.clone(), pipe, is_running.clone());
        setup_sig_handler(tx.clone())?;
        let handler = run_as_daemon(tr, config, is_running.clone());
        Ok(Self {
            translate_thread: join_handle,
            translate_should_run: is_running,
            handler_thread: handler,
            args,
        })
    }

    pub fn wait_for_exit(self) -> crate::error::Result<()> {
        let e1 = self.handler_thread.join();
        let e2 = self.translate_thread.join();
        if e1.is_err() || e2.is_err() {
            return Err(crate::error::Error::JoinError.into());
        }
        Ok(())
    }
}

fn start_daemon(config: Configuration, args: Args) -> crate::error::Result<()> {
    let state = DaemonState::create_from_config(config, args)?;
    state.wait_for_exit()
}

fn setup_automatic_trigger(
    config: AutomaticConfig,
    sender: Sender<Action>,
    is_running: Arc<AtomicBool>,
) -> crate::error::Result<JoinHandle<()>> {
    let mut scheduler = Scheduler::new(Coordinates::new(config.latitude, config.longitude).ok_or(
        crate::error::Error::InvalidCoordinates(config.latitude, config.longitude),
    )?);

    if !config.on_dawn.is_empty() {
        scheduler.add_action(scheduler::ActionTrigger::Dawn, config.on_dawn.clone());
    }

    if !config.on_sunrise.is_empty() {
        scheduler.add_action(scheduler::ActionTrigger::Sunrise, config.on_sunrise.clone());
    }
    if !config.on_dusk.is_empty() {
        scheduler.add_action(scheduler::ActionTrigger::Dusk, config.on_dusk.clone());
    }
    if !config.on_sunset.is_empty() {
        scheduler.add_action(scheduler::ActionTrigger::Sunset, config.on_sunset.clone());
    }
    Ok(std::thread::spawn(move || {
        while is_running.load(std::sync::atomic::Ordering::SeqCst) {
            let now = Utc::now();
            let (trigger, at) = scheduler.estimated_next_event_at(now);
            let duration = at - now;
            if duration.abs() > TimeDelta::seconds(30) {
                sleep(30);
            }
            if duration.abs() < TimeDelta::seconds(10) {
                let action = scheduler.get_action(trigger);
                if let Some(str) = action {
                    sender
                        .send(Action::Trigger { action: str })
                        .expect("Failed to send trigger")
                }
            }
        }
    }))
}

fn setup_manual_trigger(config: ManualConfig, is_running: Arc<AtomicBool>) -> crate::error::Result<JoinHandle<()>> {
    let scheduler = Scheduler::new()
    Ok(std::thread::spawn(move || {
        while is_running.load(std::sync::atomic::Ordering::SeqCst) {
            let now = Utc::now();
            
            let (trigger, at) = scheduler.estimated_next_event_at(now);
            let duration = at - now;
            if duration.abs() > TimeDelta::seconds(30) {
                sleep(30);
            }
            if duration.abs() < TimeDelta::seconds(10) {
                let action = scheduler.get_action(trigger);
                if let Some(str) = action {
                    sender
                        .send(Action::Trigger { action: str })
                        .expect("Failed to send trigger")
                }
            }
        }
    }))
}


fn start_translate_pipe_events(
    sender: Sender<Action>,
    mut pipe: File,
    is_running: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        while let Ok(action) = bincode::decode_from_std_read(&mut pipe, bincode::config::standard())
        {
            sender.send(action).expect("Failed to send action");
            if !is_running.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
        }
    })
}

fn init() -> crate::error::Result<(Configuration, Args)> {
    let args = Args::parse();
    let config = load_config(args.config.clone())?;
    Ok((config, args))
}

fn load_config(path: String) -> crate::error::Result<Configuration> {
    let config: Configuration = Figment::new()
        .merge(Toml::file(path))
        .merge(Serialized::defaults(Configuration::default()))
        .extract()?;
    Ok(config)
}

fn setup_sig_handler(sender: Sender<Action>) -> crate::error::Result<()> {
    ctrlc::set_handler(move || {
        sender
            .send(Action::Stop)
            .expect("Failed to Stop daemon on handler")
    })?;
    Ok(())
}

fn run_as_daemon(
    events: Receiver<Action>,
    mut config: Configuration,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        while let Ok(c) = events.recv() {
            if c == Action::Stop {
                stop.store(false, std::sync::atomic::Ordering::SeqCst);
                break;
            }
            handle_command(c, &mut config).expect("failed to handle command")
        }
        std::fs::remove_file(&config.pipe).expect("Failed to remove file");
    })
}

fn handle_command(command: Action, config: &mut Configuration) -> crate::error::Result<()> {
    match command {
        Action::Stop => {
            unreachable!("this should never happen!")
        }
        Action::Enable => config.enabled = true,
        Action::Disable => config.enabled = false,
        Action::Toggle => config.enabled = !config.enabled,
        Action::ReloadConfig { path } => *config = load_config(path)?,
        Action::Trigger { action } => todo!(),
    };
    Ok(())
}

#[cfg(test)]
mod test {}
