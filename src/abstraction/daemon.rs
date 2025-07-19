use std::{
    borrow::Cow,
    io::{self, BufReader},
    path::PathBuf,
    sync::mpsc::{Receiver, Sender, channel},
    thread::{JoinHandle, sleep},
    time::Duration,
};

use crate::{Args, Commands};
use chrono::{TimeDelta, Utc};
use interprocess::local_socket::{
    GenericNamespaced, Listener, ListenerOptions, Stream, ToNsName, traits::ListenerExt,
};
use notify::{INotifyWatcher, RecommendedWatcher, Watcher, recommended_watcher};

use super::{
    actions::Action,
    config::{Configuration, SOCKET_NAME, load_config},
    scheduler::{EventSource, Scheduler},
};

pub struct Context {
    config: Configuration,
    config_path: String,
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
                Commands::Disable => self.create_cli(),
                Commands::Enable => self.create_cli(),
                Commands::Toggle => self.create_cli(),
                Commands::Stop => self.create_cli(),
                Commands::Start => self.create_daemon(),
                Commands::Info => self.create_cli(),
                Commands::GenerateDefaultConfig => self.create_cli(),
            },
            None => ,
        }
    }

    fn wait_for_exit(mut self, mut state: Daemon) -> crate::error::Result<()> {
        while let Ok(c) = state.receiver.recv() {
            if c == Action::Stop {
                break;
            }
            state = handle_command(c, &mut self.config, state).expect("failed to handle command");
        }
        Ok(())
    }

    pub fn run(self, args: Args) -> Result<(), Box<dyn std::error::Error + 'static>> {
        let state = create_state(&self.config)?;
        match state {
            AppState::Daemon(mut daemon) => {
                setup_sig_handler(daemon.sender.clone())?;
                daemon.handle_cmd_opt(args.command, &self.config)?;
                self.wait_for_exit(daemon)?;
            }
            AppState::Cli(mut cli) => {
                println!("sending command to daemon");
                cli.handle_cmd_opt(args.command, &self.config)?;
            }
        }

        Ok(())
    }
}
impl CommandHandler for AppState {
    fn handle_cmd(
        &mut self,
        command: Commands,
        config: &Configuration,
    ) -> crate::error::Result<()> {
        match self {
            AppState::Daemon(daemon) => daemon.handle_cmd(command, config),
            AppState::Cli(cli) => cli.handle_cmd(command, config),
        }
    }
}
pub trait CommandHandler {
    fn handle_cmd(&mut self, command: Commands, config: &Configuration)
    -> crate::error::Result<()>;
    fn handle_cmd_opt(
        &mut self,
        command: Option<Commands>,
        config: &Configuration,
    ) -> crate::error::Result<()> {
        if let Some(cmd) = command {
            self.handle_cmd(cmd, config)?;
        }
        Ok(())
    }
}

pub struct Daemon {
    watcher: Option<INotifyWatcher>,
    sender: Sender<Action>,
    receiver: Receiver<Action>,
    config_sender: Sender<Configuration>,
}

impl Daemon {
    fn recreate(mut self, config: &mut Configuration) -> crate::error::Result<Self> {
        if !config.hot_reload {
            self.watcher = None;
        } else if config.hot_reload && self.watcher.is_none() {
            self.watcher = Some(start_hot_reload(
                config.config_path.clone(),
                self.sender.clone(),
            )?);
        }
        self.config_sender.send(config.clone())?;

        Ok(self)
    }
}

pub struct Cli {
    stream: Stream,
}

impl CommandHandler for Daemon {
    fn handle_cmd(
        &mut self,
        command: Commands,
        config: &Configuration,
    ) -> crate::error::Result<()> {
        if command == Commands::Info {
            println!("{config}")
        } else {
            self.sender.send(command.to_action())?;
        }
        Ok(())
    }
}

impl CommandHandler for Cli {
    fn handle_cmd(
        &mut self,
        command: Commands,
        config: &Configuration,
    ) -> crate::error::Result<()> {
        let bincode_config = bincode::config::standard();
        match command {
            Commands::Disable => {
                bincode::encode_into_std_write(Action::Disable, &mut self.stream, bincode_config)?;
            }
            Commands::Enable => {
                bincode::encode_into_std_write(Action::Enable, &mut self.stream, bincode_config)?;
            }
            Commands::Toggle => {
                bincode::encode_into_std_write(Action::Toggle, &mut self.stream, bincode_config)?;
            }
            Commands::Stop => {
                bincode::encode_into_std_write(Action::Stop, &mut self.stream, bincode_config)?;
            }
            Commands::Info => {
                println!("{config}");
            }
        };
        Ok(())
    }
}
pub enum AppState {
    Daemon(Daemon),
    Cli(Cli),
}

impl AppState {
    pub fn run(self, context: Context) -> crate::error::Result<()> {
        match self {
            AppState::Daemon(daemon) => daemon.run(context),
            AppState::Cli(cli) => cli.run(contexxt),
        }
    }
}

fn create_state(config: &Configuration) -> crate::error::Result<AppState> {
    let (sender, receiver) = channel();
    let (sender_config, receiver_config) = channel();
    let name = SOCKET_NAME.to_ns_name::<GenericNamespaced>()?;
    let opts = ListenerOptions::new().name(name.clone());
    if let Ok(listener) = opts.create_sync() {
        let sc = sender.clone();
        std::thread::spawn(move || start_translate_events(sc, listener));
        let _trigger_thread = setup_trigger(sender.clone(), receiver_config)?;
        sender_config.send(config.clone())?;
        let mut watcher = None;
        if config.hot_reload {
            watcher = Some(start_hot_reload(
                config.config_path.clone(),
                sender.clone(),
            )?);
        }
        Ok(AppState::Daemon(Daemon {
            watcher,
            sender,
            receiver,
            config_sender: sender_config,
        }))
    } else {
        let stream = <Stream as interprocess::local_socket::traits::Stream>::connect(name)?;
        Ok(AppState::Cli(Cli { stream }))
    }
}

fn start_hot_reload(
    config_path: PathBuf,
    sender: Sender<Action>,
) -> crate::error::Result<RecommendedWatcher> {
    let mut watcher = recommended_watcher(move |ev: Result<notify::Event, notify::Error>| {
        if let Ok(e) = ev {
            if let Some(_src) = e.source() {
                sender
                    .send(Action::ReloadConfig)
                    .expect("Failed to send hot reload event");
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
    loop {
        match receiver.try_recv() {
            Ok(config) => scheduler = create_scheduler(&config),
            Err(try_err) => match try_err {
                std::sync::mpsc::TryRecvError::Empty => {}
                std::sync::mpsc::TryRecvError::Disconnected => return Ok(()),
            },
        }
        if let Some(source) = &mut scheduler {
            let now = Utc::now();
            if let Some((action, at)) = source.next_event_at(now) {
                let duration = at - now;
                if duration.abs() > TimeDelta::seconds(30) {
                    sleep(Duration::from_secs(10));
                }
                if duration.abs() < TimeDelta::seconds(10) {
                    sender.send(Action::Trigger { action })?;
                }
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
        eprintln!("Invalid configuration");
        None
    }
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
            .expect("Failed to Stop daemon on handler")
    })?;
    Ok(())
}

fn handle_command(
    command: Action,
    config: &mut Configuration,
    mut daemon: Daemon,
) -> crate::error::Result<Daemon> {
    println!("handling : {command}");
    match command {
        Action::Stop => {
            unreachable!("this should never happen!")
        }
        Action::Enable => config.enabled = true,
        Action::Disable => config.enabled = false,
        Action::Toggle => config.enabled = !config.enabled,
        Action::ReloadConfig => {
            println!("Reloading");
            *config = load_config(config.config_path.display().to_string())?;
            daemon = daemon.recreate(config)?;
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
