use std::io;
use std::path::{Path, PathBuf};

use viewer_media::{Kind, Media};

#[derive(Debug)]
pub(crate) struct OpenFile {
    path: PathBuf,
    name: String,
    media: io::Result<Option<Media>>,
}

impl OpenFile {
    pub(crate) fn open(path: PathBuf) -> Self {
        let media = viewer_media::probe(&path);
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        Self { path, name, media }
    }

    pub(crate) fn empty() -> Self {
        Self {
            path: PathBuf::new(),
            name: String::new(),
            media: Ok(None),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn media(&self) -> Option<Media> {
        *self.media.as_ref().ok()?
    }

    pub(crate) fn kind(&self) -> Option<Kind> {
        self.media().map(Media::kind)
    }

    pub(crate) fn error(&self) -> Option<&io::Error> {
        self.media.as_ref().err()
    }
}
