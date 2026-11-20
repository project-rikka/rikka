use std::path::Path;

use rikka_media_format::{Format, Kind as Media};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    Still,
    Motion,
}

impl Kind {
    #[must_use]
    pub(crate) fn from_path(path: &Path) -> Option<Kind> {
        match Format::from_path(path)?.kind() {
            Media::Picture => Some(Kind::Still),
            Media::Movie => Some(Kind::Motion),
            Media::Sound => None,
        }
    }
}
