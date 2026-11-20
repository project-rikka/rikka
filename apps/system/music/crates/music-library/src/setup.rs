use std::path::{Path, PathBuf};

use rikka_dirs::{UserDir, UserDirs};

use crate::error::{Error, Result};
use crate::library::Library;

const MUSIC: &str = "Music";

#[derive(Clone, Debug)]
#[must_use]
pub struct Setup {
    pub(crate) database: PathBuf,
    pub(crate) artwork: PathBuf,
    pub(crate) music: Option<PathBuf>,
    pub(crate) excluded: Vec<PathBuf>,
}

impl Setup {
    pub fn new(database: impl Into<PathBuf>, artwork: impl Into<PathBuf>) -> Setup {
        Setup {
            database: database.into(),
            artwork: artwork.into(),
            music: None,
            excluded: Vec::new(),
        }
    }

    pub fn music_folder(mut self, folder: impl Into<PathBuf>) -> Setup {
        self.music = Some(folder.into());
        self
    }

    pub fn exclude(mut self, folder: impl Into<PathBuf>) -> Setup {
        self.excluded.push(folder.into());
        self
    }

    pub fn open(self) -> Result<Library> {
        Library::open(self)
    }

    pub(crate) fn music(&self) -> Result<PathBuf> {
        let music = self.music.clone().unwrap_or_else(default_music_folder);
        absolute(&music)?;
        Ok(music)
    }
}

#[must_use]
fn default_music_folder() -> PathBuf {
    let dirs = UserDirs::load();
    dirs.path(UserDir::Music)
        .or_else(|| dirs.home().map(|home| home.join(MUSIC)))
        .unwrap_or_else(|| PathBuf::from("/").join(MUSIC))
}

pub(crate) fn absolute(path: &Path) -> Result<()> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(Error::Folder(path.to_path_buf()))
    }
}
