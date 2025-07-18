use std::{
    fs::File,
    path::{Path, PathBuf},
    str::FromStr,
    sync::mpsc::{Receiver, Sender, channel},
    thread::{JoinHandle, sleep},
    time::Duration,
};

use chrono::{TimeDelta, Utc};
use notify::{INotifyWatcher, RecommendedWatcher, Watcher, recommended_watcher};

use crate::{Args, platform};

use super::{
    actions::Action,
    config::{Configuration, load_config},
    scheduler::{EventSource, Scheduler},
};

pub fn check_daemon_running<P: AsRef<Path>>(path: P) -> crate::error::Result<bool> {
    Ok(std::fs::exists(path)?)
}

pub struct DaemonState {
    args: Args,
    config: Configuration,
}

impl DaemonState {
    pub fn create_from_config(config: Configuration, args: Args) -> crate::error::Result<Self> {
        println!("Config: {config:?}");
        Ok(Self { config, args })
    }

    pub fn wait_for_exit(mut self) -> crate::error::Result<()> {
        let mut state = setup_handlers(PathBuf::from_str(&self.args.config)?, &self.config)?;
        setup_sig_handler(state.sender.clone())?;

        while let Ok(c) = state.receiver.recv() {
            if c == Action::Stop {
                std::fs::remove_file(&self.config.pipe).expect("Failed to remove file");
                state.wait_for_exit();
                break;
            }
            state = handle_command(c, &mut self.config, state).expect("failed to handle command");
        }
        Ok(())
    }
}

struct RunningState {
    pipe_path: PathBuf,
    watcher: Option<INotifyWatcher>,
    sender: Sender<Action>,
    receiver: Receiver<Action>,
}

impl RunningState {
    pub fn wait_for_exit(self) {
        drop(self.receiver);
        if let Some(mut w) = self.watcher {
            let _ = w.unwatch(&self.pipe_path);
        }
    }
}

fn setup_handlers(
    config_path: PathBuf,
    config: &Configuration,
) -> crate::error::Result<RunningState> {
    let (sender, receiver) = channel();
    let pipe = platform::make_pipe(&config.pipe)?;
    let _translate_thread = start_translate_pipe_events(sender.clone(), pipe);
    let _trigger_thread = setup_trigger(config, sender.clone())?;
    let mut watcher = None;
    if config.hot_reload {
        watcher = Some(start_hot_reload(config_path, sender.clone())?);
    }
    Ok(RunningState {
        pipe_path: config.pipe.clone(),
        watcher,
        sender,
        receiver,
    })
}

fn start_hot_reload(
    config_path: PathBuf,
    clone: Sender<Action>,
) -> crate::error::Result<RecommendedWatcher> {
    let mut watcher = recommended_watcher(move |ev: Result<notify::Event, notify::Error>| {
        if let Ok(e) = ev {
            if let Some(src) = e.source() {
                clone
                    .send(Action::ReloadConfig {
                        path: src.to_owned(),
                    })
                    .expect("Failed to send hot reload event");
            }
        }
    })?;
    watcher.watch(&config_path, notify::RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

pub fn start_daemon(config: Configuration, args: Args) -> crate::error::Result<()> {
    let state = DaemonState::create_from_config(config, args)?;

    state.wait_for_exit()
}
fn setup_trigger(
    config: &Configuration,
    sender: Sender<Action>,
) -> crate::error::Result<JoinHandle<()>> {
    let scheduler = create_scheduler(config);
    if let Some(source) = scheduler {
        Ok(std::thread::spawn(move || {
            loop {
                let now = Utc::now();
                if let Some((action, at)) = source.next_event_at(now) {
                    let duration = at - now;
                    if duration.abs() > TimeDelta::seconds(30) {
                        sleep(Duration::from_secs(10));
                    }
                    if duration.abs() < TimeDelta::seconds(10) {
                        sender
                            .send(Action::Trigger { action })
                            .expect("Failed to send action")
                    }
                }
            }
        }))
    } else {
        Err(crate::error::Error::InvalidConfiguration.into())
    }
}

fn create_scheduler(config: &Configuration) -> Option<Box<dyn EventSource + Send>> {
    if let Some(ref auto) = config.automatic {
        Some(Box::new(Scheduler::automatic(
            (auto.latitude, auto.longitude),
            config.actions.clone(),
        )))
    } else if let Some(ref manual) = config.manual {
        Some(Box::new(Scheduler::manual(
            manual.time_stamps.clone(),
            config.actions.clone(),
        )))
    } else {
        None
    }
}

fn start_translate_pipe_events(sender: Sender<Action>, mut pipe: File) -> JoinHandle<()> {
    std::thread::spawn(move || {
        while let Ok(action) = bincode::decode_from_std_read(&mut pipe, bincode::config::standard())
        {
            sender.send(action).expect("Failed to send action");
        }
    })
}
fn setup_sig_handler(sender: Sender<Action>) -> crate::error::Result<()> {
    ctrlc::set_handler(move || {
        sender
            .send(Action::Stop)
            .expect("Failed to Stop daemon on handler")
    })?;
    Ok(())
}

fn handle_command(
    command: Action,
    config: &mut Configuration,
    mut state: RunningState,
) -> crate::error::Result<RunningState> {
    match command {
        Action::Stop => {
            unreachable!("this should never happen!")
        }
        Action::Enable => config.enabled = true,
        Action::Disable => config.enabled = false,
        Action::Toggle => config.enabled = !config.enabled,
        Action::ReloadConfig { path } => {
            println!("Reloading {path}");
            *config = load_config(path)?;
            if config.pipe != state.pipe_path {
                state = setup_handlers(config.pipe.clone(), config)?
            }
        }
        Action::Trigger { action } => {
            if config.enabled {
                std::process::Command::new("sh")
                    .arg("-c")
                    .arg(action)
                    .spawn()?;
            }
        }
    };
    Ok(state)
}
