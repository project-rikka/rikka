#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::base;

const WALLPAPERS: &str = "Wallpaper";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UserDir {
    Desktop,
    Documents,
    Downloads,
    Music,
    Pictures,
    Videos,
    Templates,
    PublicShare,
}

impl UserDir {
    pub const ALL: [UserDir; 8] = [
        UserDir::Desktop,
        UserDir::Documents,
        UserDir::Downloads,
        UserDir::Music,
        UserDir::Pictures,
        UserDir::Videos,
        UserDir::Templates,
        UserDir::PublicShare,
    ];

    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            UserDir::Desktop => "XDG_DESKTOP_DIR",
            UserDir::Documents => "XDG_DOCUMENTS_DIR",
            UserDir::Downloads => "XDG_DOWNLOAD_DIR",
            UserDir::Music => "XDG_MUSIC_DIR",
            UserDir::Pictures => "XDG_PICTURES_DIR",
            UserDir::Videos => "XDG_VIDEOS_DIR",
            UserDir::Templates => "XDG_TEMPLATES_DIR",
            UserDir::PublicShare => "XDG_PUBLICSHARE_DIR",
        }
    }

    #[must_use]
    const fn fallback(self) -> &'static str {
        match self {
            UserDir::Desktop => "Desktop",
            UserDir::Documents => "Documents",
            UserDir::Downloads => "Downloads",
            UserDir::Music => "Music",
            UserDir::Pictures => "Pictures",
            UserDir::Videos => "Videos",
            UserDir::Templates => "Templates",
            UserDir::PublicShare => "Public",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct UserDirs {
    named: HashMap<UserDir, PathBuf>,
    home: Option<PathBuf>,
}

impl UserDirs {
    #[must_use]
    pub fn load() -> UserDirs {
        UserDirs {
            named: platform::named(),
            home: base::home().map(Path::to_path_buf),
        }
    }

    #[must_use]
    pub fn path(&self, which: UserDir) -> Option<PathBuf> {
        let named = match self.named.get(&which) {
            Some(path) => path.clone(),
            None => self.home.as_ref()?.join(which.fallback()),
        };
        (Some(named.as_path()) != self.home.as_deref()).then_some(named)
    }

    #[must_use]
    pub fn downloads(&self) -> PathBuf {
        self.path(UserDir::Downloads)
            .or_else(|| self.home.clone())
            .unwrap_or_else(std::env::temp_dir)
    }

    #[must_use]
    pub fn wallpapers(&self) -> Option<PathBuf> {
        Some(self.path(UserDir::Pictures)?.join(WALLPAPERS))
    }

    #[must_use]
    pub fn at(&self, path: &Path) -> Option<UserDir> {
        UserDir::ALL
            .into_iter()
            .find(|which| self.path(*which).as_deref() == Some(path))
    }

    #[must_use]
    pub fn home(&self) -> Option<&Path> {
        self.home.as_deref()
    }
}
