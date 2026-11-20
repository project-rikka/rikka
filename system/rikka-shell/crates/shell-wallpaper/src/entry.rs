use std::path::{Path, PathBuf};

use rikka_session::wallpaper::{Preview, Thumbnail, WallpaperEntry};

use crate::kind::Kind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    path: PathBuf,
    id: String,
    name: String,
    kind: Kind,
}

impl Entry {
    #[must_use]
    pub(crate) fn from_path(path: impl Into<PathBuf>) -> Option<Entry> {
        let path = path.into();
        let kind = Kind::from_path(&path)?;
        let id = path.file_name()?.to_str()?.to_owned();
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        Some(Entry {
            path,
            id,
            name,
            kind,
        })
    }

    #[must_use]
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub(crate) fn kind(&self) -> Kind {
        self.kind
    }

    #[must_use]
    pub(crate) fn listed(&self, thumbnail: Thumbnail) -> WallpaperEntry {
        let preview = match self.kind {
            Kind::Still => Preview::Still { thumbnail },
            Kind::Motion => Preview::Motion {
                file: self.path.clone(),
            },
        };
        WallpaperEntry {
            id: self.id.clone(),
            name: self.name.clone(),
            preview,
        }
    }
}
