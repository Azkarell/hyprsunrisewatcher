pub mod scheduler;

use std::{fs::File, path::Path};

pub struct FilePipe {
    file: File,
}

pub trait FilePipeBuilder {
    fn create_file_pipe<P: AsRef<Path>>(&self, path: P) -> FilePipe;
}
