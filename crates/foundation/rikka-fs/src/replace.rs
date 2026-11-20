use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::staged::Staged;

const DEFAULT_MODE: u32 = 0o644;
const PRIVATE_MODE: u32 = 0o600;
pub(crate) const PERMISSION_BITS: u32 = 0o7777;

pub fn replace(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    Replace::new(path.as_ref()).write(bytes)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Preserve { default: u32 },
    Fixed(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Durability {
    Synced,
    Unsynced,
}

#[must_use]
#[derive(Clone, Debug)]
pub struct Replace {
    path: PathBuf,
    mode: Mode,
    durability: Durability,
    keep_extension: bool,
}

impl Replace {
    pub fn new(path: impl Into<PathBuf>) -> Replace {
        Replace {
            path: path.into(),
            mode: Mode::Preserve {
                default: DEFAULT_MODE,
            },
            durability: Durability::Synced,
            keep_extension: false,
        }
    }

    pub fn unsynced(mut self) -> Replace {
        self.durability = Durability::Unsynced;
        self
    }

    pub fn private(mut self) -> Replace {
        self.mode = Mode::Fixed(PRIVATE_MODE);
        self
    }

    pub fn default_mode(mut self, mode: u32) -> Replace {
        self.mode = Mode::Preserve {
            default: mode & PERMISSION_BITS,
        };
        self
    }

    pub fn keep_extension(mut self) -> Replace {
        self.keep_extension = true;
        self
    }

    pub fn stage(self) -> io::Result<Staged> {
        Staged::create(&self.path, self.mode, self.durability, self.keep_extension)
    }

    pub fn write(self, bytes: impl AsRef<[u8]>) -> io::Result<()> {
        let mut staged = self.stage()?;
        staged.write_all(bytes.as_ref())?;
        staged.commit()
    }
}
