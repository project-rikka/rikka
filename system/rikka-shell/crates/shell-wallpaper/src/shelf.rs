use std::path::Path;
use std::sync::Arc;

use rikka_seam::Publisher;
use rikka_session::wallpaper::{DisplayedWallpaper, LISTED, Thumbnail, WallpaperEntry};

use crate::error::{Error, Result};
use crate::library::Library;
use crate::library_watcher::LibraryWatcher;
use crate::thumbnails::Thumbnails;

#[derive(Clone, Debug)]
pub(crate) struct Shelf {
    library: Result<Library, &'static str>,
    thumbnails: Option<Thumbnails>,
    publisher: Option<Publisher>,
}

impl Shelf {
    #[must_use]
    pub(crate) fn new() -> Shelf {
        let thumbnails = Thumbnails::shared();
        if thumbnails.is_none() {
            tracing::warn!("no thumbnail cache for wallpapers");
        }
        Shelf::from_parts(Library::session().cloned(), thumbnails)
    }

    #[must_use]
    fn from_parts(library: Result<Library, &'static str>, thumbnails: Option<Thumbnails>) -> Shelf {
        Shelf {
            library,
            thumbnails,
            publisher: None,
        }
    }

    #[must_use]
    pub(crate) fn announcing(&mut self, publisher: Publisher) -> LibraryWatcher {
        self.publisher = Some(publisher);
        let library_watcher = LibraryWatcher::new(self.clone());
        if let Some(thumbnails) = &self.thumbnails {
            let asking = library_watcher.clone();
            thumbnails.announcing(Arc::new(move || asking.announce()));
        }
        library_watcher
    }

    pub(crate) fn listing(&self) -> Result<Vec<WallpaperEntry>> {
        let library = self.library.clone().map_err(Error::Unavailable)?;
        let entries = library.list()?;
        let small = match &self.thumbnails {
            Some(thumbnails) => thumbnails.settle(&entries),
            None => vec![Thumbnail::None; entries.len()],
        };
        Ok(entries
            .iter()
            .zip(small)
            .map(|(entry, thumbnail)| entry.listed(thumbnail))
            .collect())
    }

    pub(crate) fn announce(&self) {
        let Some(publisher) = &self.publisher else {
            return;
        };
        match self.listing() {
            Ok(listed) => {
                if let Err(error) = publisher.emit(LISTED, &listed) {
                    tracing::warn!("failed to announce wallpapers: {error}");
                }
            }
            Err(error) => tracing::warn!("failed to read wallpapers: {error}"),
        }
    }

    #[must_use]
    pub(crate) fn folder(&self) -> Option<&Path> {
        self.library.as_ref().ok().map(Library::folder)
    }

    #[must_use]
    pub(crate) fn shown(&self, id: &str) -> Option<DisplayedWallpaper> {
        let entry = self.library.as_ref().ok()?.at_id(id)?;
        Some(DisplayedWallpaper {
            id: entry.id().to_owned(),
            file: entry.path().to_path_buf(),
        })
    }
}
