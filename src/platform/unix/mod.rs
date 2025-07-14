use std::{os::unix::ffi::OsStrExt, path::Path};

use nix::{errno::Errno, libc::mkfifo};

use crate::error::{Error, Result};

pub fn create_pipe<P: AsRef<Path>>(path: P) -> Result<()> {
    unsafe {
        let os_str = path.as_ref().as_os_str().as_bytes();
        let res = mkfifo(os_str.as_ptr() as *const i8, 0o0666);
        if res == -1 {
            let errno = Errno::last();
            Err(Error::Errno(errno).into())
        } else {
            Ok(())
        }
    }
}
