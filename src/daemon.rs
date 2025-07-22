use std::{
    io::{self, BufReader},
    path::PathBuf,
    sync::mpsc::{Receiver, Sender, channel},
    thread::{JoinHandle, sleep},
    time::Duration,
};

use crate::{context::Context, scheduler::EventCache};
use chrono::{TimeDelta, Utc};
use interprocess::local_socket::{
    GenericNamespaced, Listener, ListenerOptions, Stream, ToNsName, traits::ListenerExt,
};
use notify::{INotifyWatcher, RecommendedWatcher, Watcher, recommended_watcher};

use crate::{
    actions::Action,
    config::{Configuration, SOCKET_NAME},
    scheduler::{EventSource, TriggerSource},
};

pub struct Daemon {
    pub watcher: Option<INotifyWatcher>,
    pub sender: Sender<Action>,
    pub receiver: Receiver<Action>,
    pub config_sender: Sender<Configuration>,
}

impl Daemon {
    fn recreate(
        mut self,
        config: &mut Configuration,
        config_path: PathBuf,
    ) -> crate::error::Result<Self> {
        if !config.hot_reload {
            self.watcher = None;
        } else {
            self.watcher = Some(start_hot_reload(config_path, self.sender.clone())?);
        }
        self.config_sender.send(config.clone())?;

        Ok(self)
    }

    pub fn run(mut self, mut context: Context) -> crate::error::Result<()> {
        while let Ok(c) = self.receiver.recv() {
            if c == Action::Stop {
                break;
            }
            self = handle_command(c, &mut context.config, self, &context.config_path)?;
        }
        Ok(())
    }

    pub fn create(context: &Context) -> crate::error::Result<Self> {
        let (sender, receiver) = channel();
        let (sender_config, receiver_config) = channel();
        let name = SOCKET_NAME.to_ns_name::<GenericNamespaced>()?;
        let opts = ListenerOptions::new().name(name.clone());
        setup_sig_handler(sender.clone())?;
        if let Ok(listener) = opts.create_sync() {
            let sc = sender.clone();
            std::thread::spawn(move || start_translate_events(sc, listener));
            let _trigger_thread = setup_trigger(sender.clone(), receiver_config)?;
            sender_config.send(context.config.clone())?;
            let mut watcher = None;
            if context.config.hot_reload {
                watcher = Some(start_hot_reload(
                    context.config_path.clone().into(),
                    sender.clone(),
                )?);
            }
            Ok(Daemon {
                watcher,
                sender,
                receiver,
                config_sender: sender_config,
            })
        } else {
            Err(crate::error::Error::FailedtoCreateDaemon.into())
        }
    }
}

fn start_hot_reload(
    config_path: PathBuf,
    sender: Sender<Action>,
) -> crate::error::Result<RecommendedWatcher> {
    let mut watcher = recommended_watcher(move |ev: Result<notify::Event, notify::Error>| {
        if let Ok(e) = ev {
            if let notify::EventKind::Modify(_) = e.kind {
                sender
                    .send(Action::ReloadConfig)
                    .expect("failed to send hot reload event");
            }
        }
    })?;
    watcher.watch(&config_path, notify::RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

fn run_trigger_thread(
    sender: Sender<Action>,
    receiver: Receiver<Configuration>,
) -> crate::error::Result<()> {
    let mut scheduler = None;
    let mut cache = EventCache::new();
    loop {
        match receiver.try_recv() {
            Ok(config) => scheduler = Some(TriggerSource::from_config(&config)?),
            Err(try_err) => match try_err {
                std::sync::mpsc::TryRecvError::Empty => {}
                std::sync::mpsc::TryRecvError::Disconnected => return Ok(()),
            },
        }
        if let Some(source) = &mut scheduler {
            let now = Utc::now();
            if let Some(action) = source.should_trigger(now, &mut cache) {
                sender.send(Action::Trigger { action })?;
            } else {
                sleep(Duration::from_secs(25))
            }
        }
    }
}

fn setup_trigger(
    sender: Sender<Action>,
    receiver: Receiver<Configuration>,
) -> crate::error::Result<JoinHandle<()>> {
    Ok(std::thread::spawn(move || {
        run_trigger_thread(sender, receiver).expect("Failed to gracefully shutdown thread")
    }))
}

fn handle_error(conn: io::Result<Stream>) -> Option<Stream> {
    match conn {
        Ok(s) => Some(s),
        Err(err) => {
            eprintln!("Incoming connection failed: {err}");
            None
        }
    }
}
fn start_translate_events(sender: Sender<Action>, socket: Listener) {
    for conn in socket.incoming().filter_map(handle_error) {
        let mut bufread = BufReader::new(conn);
        let s = sender.clone();
        std::thread::spawn(move || {
            while let Ok(action) =
                bincode::decode_from_std_read(&mut bufread, bincode::config::standard())
            {
                s.send(action).expect("Failed to send action");
            }
        });
    }
}
fn setup_sig_handler(sender: Sender<Action>) -> crate::error::Result<()> {
    ctrlc::set_handler(move || {
        sender
            .send(Action::Stop)
            .expect("Failed to Stop daemon on sig handler")
    })?;
    Ok(())
}

fn handle_command(
    command: Action,
    config: &mut Configuration,
    mut daemon: Daemon,
    config_path: &str,
) -> crate::error::Result<Daemon> {
    match command {
        Action::Stop => {
            unreachable!("this should never happen!")
        }
        Action::Enable => config.enabled = true,
        Action::Disable => config.enabled = false,
        Action::Toggle => config.enabled = !config.enabled,
        Action::ReloadConfig => {
            if std::fs::OpenOptions::new()
                .write(false)
                .read(true)
                .open(config_path)
                .is_err()
            {
                sleep(Duration::from_millis(100));
                daemon.sender.send(Action::ReloadConfig)?;
                return Ok(daemon);
            }

            *config = Configuration::load(config_path)?;
            daemon = daemon.recreate(config, config_path.into())?;
        }
        Action::Trigger { action } => {
            if config.enabled {
                std::process::Command::new("sh")
                    .arg("-c")
                    .arg(action)
                    .spawn()?;
            }
        }
        Action::Nothing => {}
    };
    Ok(daemon)
}
