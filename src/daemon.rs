use std::{
    fmt::Display,
    io::{self, BufReader},
    path::PathBuf,
    sync::mpsc::{Receiver, Sender, channel},
    thread::{JoinHandle, sleep},
    time::Duration,
};

use crate::{Args, Commands, context::Context, info::InfoGatherer};
use chrono::{TimeDelta, Utc};
use interprocess::local_socket::{
    GenericNamespaced, Listener, ListenerOptions, Stream, ToNsName, traits::ListenerExt,
};
use notify::{INotifyWatcher, RecommendedWatcher, Watcher, recommended_watcher};

use crate::{
    actions::Action,
    config::{Configuration, SOCKET_NAME, load_config},
    scheduler::{EventSource, TriggerSource},
};

//impl CommandHandler for AppState {
//    fn handle_cmd(
//        &mut self,
//        command: Commands,
//        config: &Configuration,
//    ) -> crate::error::Result<()> {
//        match self {
//            AppState::Daemon(daemon) => daemon.handle_cmd(command, config),
//            AppState::Cli(cli) => cli.handle_cmd(command, config),
//        }
//    }
//}
// pub trait CommandHandler {
//     fn handle_cmd(&mut self, command: Commands, config: &Configuration)
//     -> crate::error::Result<()>;
//     fn handle_cmd_opt(
//         &mut self,
//         command: Option<Commands>,
//         config: &Configuration,
//     ) -> crate::error::Result<()> {
//         if let Some(cmd) = command {
//             self.handle_cmd(cmd, config)?;
//         }
//         Ok(())
//     }
// }

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
        } else if config.hot_reload && self.watcher.is_none() {
            self.watcher = Some(start_hot_reload(config_path, self.sender.clone())?);
        }
        self.config_sender.send(config.clone())?;

        Ok(self)
    }

    fn run(mut self, mut context: Context) -> crate::error::Result<()> {
        while let Ok(c) = self.receiver.recv() {
            if c == Action::Stop {
                break;
            }
            self = handle_command(c, &mut context.config, self, context.config_path.clone())
                .expect("failed to handle command");
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

//pub struct Cli {
//    stream: Stream,
//}
//
//impl CommandHandler for Daemon {
//    fn handle_cmd(
//        &mut self,
//        command: Commands,
//        config: &Configuration,
//    ) -> crate::error::Result<()> {
//        Ok(())
//    }
//}

//impl CommandHandler for Cli {
//    fn handle_cmd(
//        &mut self,
//        command: Commands,
//        config: &Configuration,
//    ) -> crate::error::Result<()> {
//        let bincode_config = bincode::config::standard();
//        match command {
//            Commands::Disable => {
//                bincode::encode_into_std_write(Action::Disable, &mut self.stream, bincode_config)?;
//            }
//            Commands::Enable => {
//                bincode::encode_into_std_write(Action::Enable, &mut self.stream, bincode_config)?;
//            }
//            Commands::Toggle => {
//                bincode::encode_into_std_write(Action::Toggle, &mut self.stream, bincode_config)?;
//            }
//            Commands::Stop => {
//                bincode::encode_into_std_write(Action::Stop, &mut self.stream, bincode_config)?;
//            }
//            Commands::Info => {
//                println!("{config}");
//            }
//        };
//        Ok(())
//    }
//}
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

//fn create_state(config: &Configuration) -> crate::error::Result<AppState> {
//    let (sender, receiver) = channel();
//    let (sender_config, receiver_config) = channel();
//    let name = SOCKET_NAME.to_ns_name::<GenericNamespaced>()?;
//    let opts = ListenerOptions::new().name(name.clone());
//    if let Ok(listener) = opts.create_sync() {
//        let sc = sender.clone();
//        std::thread::spawn(move || start_translate_events(sc, listener));
//        let _trigger_thread = setup_trigger(sender.clone(), receiver_config)?;
//        sender_config.send(config.clone())?;
//        let mut watcher = None;
//        if config.hot_reload {
//            watcher = Some(start_hot_reload(
//                config.config_path.clone(),
//                sender.clone(),
//            )?);
//        }
//        Ok(AppState::Daemon(Daemon {
//            watcher,
//            sender,
//            receiver,
//            config_sender: sender_config,
//        }))
//    } else {
//        let stream = <Stream as interprocess::local_socket::traits::Stream>::connect(name)?;
//        Ok(AppState::Cli(Cli { stream }))
//    }
//}

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
            Ok(config) => scheduler = Some(TriggerSource::from_config(&config)?),
            Err(try_err) => match try_err {
                std::sync::mpsc::TryRecvError::Empty => {}
                std::sync::mpsc::TryRecvError::Disconnected => return Ok(()),
            },
        }
        if let Some(source) = &mut scheduler {
            let now = Utc::now();
            if let Some(ev) = source.next_event_at(now) {
                if let Some(action) = ev.action {
                    let duration = ev.at - now;
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
    config_path: String,
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
            *config = load_config(config_path.clone())?;
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
