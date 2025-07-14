use crate::error::Result;
use std::fs::File;
use std::path::Path;
#[cfg(unix)]
pub mod unix;

pub fn make_pipe<P: AsRef<Path>>(path: P) -> Result<File> {
    #[cfg(unix)]
    crate::platform::unix::create_pipe(path.as_ref())?;
    #[cfg(not(unix))]
    not_implemented!("not implemented yet");
    File::options()
        .write(true)
        .read(true)
        .open(path)
        .map_err(|e| e.into())
}
