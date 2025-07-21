#[derive(Debug)]
pub enum Error {
    InvalidCoordinates(f64, f64),
    InvalidAction(String),
    InvalidConfiguration,
    FailedtoCreateDaemon,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidCoordinates(lat, long) => {
                f.write_str(&format!("Invalid Coordinates - lat: {lat} long: {long}",))
            }
            Error::InvalidAction(action) => f.write_str(&format!("Invalid action: {action}")),
            Error::InvalidConfiguration => f.write_str("Invalid configuration"),
            Error::FailedtoCreateDaemon => todo!(),
        }
    }
}

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
