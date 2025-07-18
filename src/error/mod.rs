use std::path::PathBuf;

#[cfg(unix)]
use nix::errno::Errno;

#[derive(Debug)]
pub enum Error {
    #[cfg(unix)]
    Errno(Errno),
    DaemonNotRuning,
    DaemonAlreadyRunning,
    FailedToCreatePipe(PathBuf),
    JoinError,
    InvalidCoordinates(f64, f64),
    InvalidAction(String),
    InvalidConfiguration,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Errno(errno) => errno.fmt(f),
            Error::DaemonNotRuning => f.write_str("Daemon not running"),
            Error::DaemonAlreadyRunning => f.write_str("Daemon already running"),
            Error::FailedToCreatePipe(p) => {
                f.write_str(&format!("Failed to create pipe at: {}", p.display()))
            }
            Error::JoinError => f.write_str("Failed to join worker threads"),
            Error::InvalidCoordinates(lat, long) => {
                f.write_str(&format!("Invalid Coordinates - lat: {lat} long: {long}",))
            }
            Error::InvalidAction(action) => f.write_str(&format!("Invalid action: {action}")),
            Error::InvalidConfiguration => f.write_str("Invalid configuration"),
        }
    }
}

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
