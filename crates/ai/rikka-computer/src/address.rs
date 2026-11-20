use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::environment::Environment;
use crate::error::{Error, Result};

const DISPLAY_PREFIX: &str = "rikka-computer";

const ENVIRONMENT_FILE: &str = "session.env";

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub(crate) struct Address {
    display: String,
    runtime_dir: PathBuf,
    sessions_dir: PathBuf,
}

impl Address {
    pub(crate) fn allocate() -> Result<Address> {
        let runtime_dir = rikka_dirs::runtime_dir().ok_or(Error::NoRuntimeDir)?;
        let sessions_dir = rikka_seam::net::sessions_directory().ok_or(Error::NoRuntimeDir)?;

        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Ok(Address {
            display: format!("{DISPLAY_PREFIX}-{}-{id}", std::process::id()),
            runtime_dir,
            sessions_dir,
        })
    }

    pub(crate) fn display(&self) -> &str {
        &self.display
    }

    pub(crate) fn socket(&self) -> PathBuf {
        self.runtime_dir.join(&self.display)
    }

    pub(crate) fn session_dir(&self) -> PathBuf {
        self.sessions_dir.join(&self.display)
    }

    pub(crate) fn environment_file(&self) -> PathBuf {
        self.session_dir().join(ENVIRONMENT_FILE)
    }

    pub(crate) fn environment(&self) -> Result<Environment> {
        let contents = match std::fs::read(self.environment_file()) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(Error::NoEnvironment);
            }
            Err(error) => return Err(Error::Io(error)),
        };
        Ok(Environment::new(
            contents
                .split(|byte| *byte == b'\n')
                .filter_map(parse_assignment)
                .collect(),
        ))
    }
}

fn parse_assignment(line: &[u8]) -> Option<(OsString, OsString)> {
    let equals = line.iter().position(|byte| *byte == b'=')?;
    let name = OsStr::from_bytes(&line[..equals]);
    if name.is_empty() {
        return None;
    }
    Some((
        name.to_owned(),
        OsString::from_vec(line[equals + 1..].to_vec()),
    ))
}
