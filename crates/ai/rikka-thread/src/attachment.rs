use std::path::Path;
use std::sync::Arc;

use rikka_llm::{Content, Document, Image, Media, Picture};

use crate::error::{Error, Result};

pub(crate) const LARGEST: u64 = 3 * 1024 * 1024;

const PDF: &[u8] = b"%PDF-";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    name: String,
    media: Media,
    extent: Option<(u32, u32)>,
    data: Arc<[u8]>,
}

impl Attachment {
    pub fn new(name: impl Into<String>, media: Media, data: impl Into<Arc<[u8]>>) -> Result<Self> {
        let name = name.into();
        let data = data.into();
        if data.len() as u64 > LARGEST {
            return Err(too_large(&name, data.len() as u64));
        }
        Ok(Attachment {
            extent: extent(&name, media, &data)?,
            name,
            media,
            data,
        })
    }

    pub fn read(path: &Path) -> Result<Self> {
        Attachment::read_as(path, leaf(path))
    }

    pub fn read_as(path: &Path, name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        let about = std::fs::metadata(path)
            .map_err(|error| Error::Unattachable(format!("{name} could not be read: {error}")))?;
        if about.is_dir() {
            return Err(Error::Unattachable(format!("{name} is a folder")));
        }
        if about.len() > LARGEST {
            return Err(too_large(&name, about.len()));
        }

        let data = std::fs::read(path)
            .map_err(|error| Error::Unattachable(format!("{name} could not be read: {error}")))?;
        let media = sniff_media(&data).ok_or_else(|| {
            Error::Unattachable(format!("{name} is not a picture, a PDF or text"))
        })?;
        Attachment::new(name, media, data)
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn media(&self) -> Media {
        self.media
    }

    #[must_use]
    pub fn extent(&self) -> Option<(u32, u32)> {
        self.extent
    }

    #[must_use]
    pub fn data(&self) -> &Arc<[u8]> {
        &self.data
    }

    #[must_use]
    pub fn content(&self) -> Content {
        match self.media.is_picture() {
            true => Content::Image(Image::inline(self.media, Arc::clone(&self.data))),
            false => Content::Document(
                Document::inline(self.media, Arc::clone(&self.data)).title(self.name.clone()),
            ),
        }
    }
}

fn sniff_media(data: &[u8]) -> Option<Media> {
    if let Some(picture) = Picture::sniff(data) {
        return Some(picture.media);
    }
    if data.starts_with(PDF) {
        return Some(Media::Pdf);
    }
    std::str::from_utf8(data)
        .is_ok()
        .then_some(Media::PlainText)
}

fn extent(name: &str, media: Media, data: &[u8]) -> Result<Option<(u32, u32)>> {
    if !media.is_picture() {
        return Ok(None);
    }
    let picture = Picture::sniff(data)
        .ok_or_else(|| Error::Unattachable(format!("{name} is not a picture")))?;
    if picture.media != media {
        return Err(Error::Unattachable(format!(
            "{name} is offered as {media} and is {}",
            picture.media
        )));
    }
    Ok(Some((picture.width, picture.height)))
}

fn leaf(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn too_large(name: &str, bytes: u64) -> Error {
    Error::Unattachable(format!(
        "{name} is {bytes} bytes, which is more than the {LARGEST} an attachment may be"
    ))
}
