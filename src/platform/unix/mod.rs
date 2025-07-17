use std::{ffi::CString, os::unix::ffi::OsStrExt, path::Path};

use nix::{errno::Errno, libc::mkfifo};

use crate::error::{Error, Result};

pub fn create_pipe<P: AsRef<Path>>(path: P) -> Result<()> {
    unsafe {
        let str = path.as_ref().to_str().unwrap().to_string();
        let cstr = CString::new(str)?;
        let res = mkfifo(cstr.as_ptr(), 0o0666);
        if res == -1 {
            let errno = Errno::last();
            Err(Error::Errno(errno).into())
        } else {
            Ok(())
        }
    }
}
