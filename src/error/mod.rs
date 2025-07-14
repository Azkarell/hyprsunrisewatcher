#[cfg(unix)]
use nix::errno::Errno;

#[derive(Debug)]
pub enum Error {
    #[cfg(unix)]
    Errno(Errno),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Errno(errno) => errno.fmt(f),
        }
    }
}

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
