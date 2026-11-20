use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

const ATTEMPTS: usize = 8;

#[derive(Debug)]
pub(crate) struct Private {
    directory: PathBuf,
    path: PathBuf,
}

impl Private {
    pub(crate) fn bind() -> Result<(Private, UnixListener)> {
        let runtime = rikka_dirs::runtime_dir().ok_or(Error::NoRuntimeDir)?;
        let directory = make_directory(&runtime)?;
        let private = Private {
            path: directory.join("wayland"),
            directory,
        };
        let listener = UnixListener::bind(&private.path).map_err(Error::Bind)?;
        listener.set_nonblocking(true).map_err(Error::Bind)?;
        Ok((private, listener))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Private {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_dir(&self.directory);
    }
}

fn make_directory(runtime: &Path) -> Result<PathBuf> {
    let mut last = None;
    for _ in 0..ATTEMPTS {
        let directory = runtime.join(format!(
            "rikka-nested-{}",
            rikka_compositor_types::unguessable()
        ));
        match DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => last = Some(error),
            Err(error) => return Err(Error::Directory(error)),
        }
    }
    Err(Error::Directory(last.unwrap_or_else(|| {
        std::io::Error::other("no free private directory name")
    })))
}
