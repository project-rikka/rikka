use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use rikka_dirs::UserDirs;

use crate::entry::Entry;
use crate::error::{Error, Result};

#[derive(Clone, Debug)]
pub(crate) struct Library {
    folder: PathBuf,
}

impl Library {
    pub(crate) fn session() -> Result<&'static Library, &'static str> {
        static LIBRARY: OnceLock<Result<Library, String>> = OnceLock::new();
        LIBRARY
            .get_or_init(|| {
                Library::resolve().map_err(|error| {
                    tracing::warn!("wallpaper folder unavailable: {error}");
                    error.to_string()
                })
            })
            .as_ref()
            .map_err(String::as_str)
    }

    pub(crate) fn resolve() -> Result<Library> {
        let folder = UserDirs::load().wallpapers().ok_or(Error::NoPictures)?;
        rikka_dirs::ensure(&folder).map_err(Error::Folder)?;
        Ok(Library { folder })
    }

    #[must_use]
    pub(crate) fn folder(&self) -> &Path {
        &self.folder
    }

    pub(crate) fn list(&self) -> Result<Vec<Entry>> {
        let entries = fs::read_dir(&self.folder).map_err(|source| Error::Read {
            path: self.folder.clone(),
            source,
        })?;
        let mut wallpapers: Vec<Entry> = entries
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|kind| !kind.is_dir()))
            .filter_map(|entry| Entry::from_path(entry.path()))
            .collect();
        wallpapers.sort_by_cached_key(|entry| (entry.name().to_lowercase(), entry.id().to_owned()));
        Ok(wallpapers)
    }

    #[must_use]
    pub(super) fn at_id(&self, id: &str) -> Option<Entry> {
        if !is_name(id) {
            return None;
        }
        let path = self.folder.join(id);
        path.is_file().then(|| Entry::from_path(path)).flatten()
    }
}

fn is_name(id: &str) -> bool {
    let mut components = Path::new(id).components();
    let first = components.next();
    components.next().is_none() && matches!(first, Some(Component::Normal(_)))
}
