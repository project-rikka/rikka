use std::sync::Arc;

use rikka_clipboard::{Change, Clipboard, PNG, Selection, URI_LIST, text_mime, uri_list};
use rikka_image::{Format, Image};

use crate::entry::{Content, Picture};
use crate::history::MAX_CLIPPING;

const PICTURES: [&str; 3] = [PNG, "image/jpeg", "image/webp"];

const THUMBNAIL_WIDTH: u32 = 512;
const THUMBNAIL_HEIGHT: u32 = 256;

pub(crate) fn capture(clipboard: &Clipboard, change: &Change) -> Option<Content> {
    if clipboard.is_secret(change) {
        tracing::debug!("skipped clipboard entry marked secret");
        return None;
    }
    if let Some(mime) = picture_type(change) {
        return picture(clipboard, mime);
    }
    if change.offers(URI_LIST) {
        let uris = uri_list(&String::from_utf8(read(clipboard, URI_LIST)?).ok()?);
        return (!uris.is_empty()).then_some(Content::Files(uris));
    }
    let mime = text_mime(change.mime_types())?;
    let text = String::from_utf8(read(clipboard, mime)?).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    Some(Content::Text(Arc::from(text)))
}

fn read(clipboard: &Clipboard, mime: &str) -> Option<Vec<u8>> {
    clipboard
        .read_within(Selection::Clipboard, mime, MAX_CLIPPING)
        .inspect_err(|error| tracing::debug!("skipped `{mime}` clipping: {error}"))
        .ok()
}

fn picture_type(change: &Change) -> Option<&str> {
    PICTURES
        .into_iter()
        .find(|mime_type| change.offers(mime_type))
        .or_else(|| {
            change
                .mime_types()
                .iter()
                .map(String::as_str)
                .find(|offered| Format::from_mime_type(offered).is_some())
        })
}

fn picture(clipboard: &Clipboard, mime: &str) -> Option<Content> {
    let data = read(clipboard, mime)?;
    let image = Image::from_bytes(&data)
        .inspect_err(|error| tracing::debug!("failed to decode clipboard picture: {error}"))
        .ok()?;
    let thumbnail = image.thumbnail(THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT).ok()?;
    let (thumbnail_width, thumbnail_height) = thumbnail.dimensions();

    Some(Content::Image(Picture {
        mime: mime.to_owned(),
        data: Arc::from(data),
        thumbnail_width,
        thumbnail_height,
        thumbnail: Arc::from(thumbnail.into_rgba8()),
    }))
}

pub(super) fn should_record(change: &Change) -> bool {
    change
        .mime_types()
        .iter()
        .any(|mime| mime.starts_with("text/") || mime.starts_with("image/"))
}
