use std::path::PathBuf;
use std::sync::Arc;

use rikka_bus::Picture;

#[derive(Clone, Debug, PartialEq)]
pub enum Image {
    Pixels(Picture),
    File(PathBuf),
    Named(String),
}

impl Image {
    #[must_use]
    pub fn named(text: &str) -> Option<Image> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        if text.starts_with("file://") {
            return rikka_open::uri_to_path(text).map(Image::File);
        }
        if text.starts_with('/') {
            return Some(Image::File(PathBuf::from(text)));
        }
        Some(Image::Named(text.to_owned()))
    }
}

#[derive(Debug)]
pub(crate) struct RawPicture {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) rowstride: i32,
    pub(crate) bits_per_sample: i32,
    pub(crate) channels: i32,
    pub(crate) data: Vec<u8>,
}

const MOST: i32 = 4096;

const DEPTH: i32 = 8;

#[must_use]
pub(crate) fn decode(raw: &RawPicture) -> Option<Picture> {
    if raw.bits_per_sample != DEPTH
        || !(1..=MOST).contains(&raw.width)
        || !(1..=MOST).contains(&raw.height)
        || !matches!(raw.channels, 3 | 4)
    {
        return None;
    }

    let channels = raw.channels as usize;
    let width = raw.width as usize;
    let height = raw.height as usize;
    let row_bytes = width.checked_mul(channels)?;
    let stride = usize::try_from(raw.rowstride).ok()?.max(row_bytes);
    let described = stride.checked_mul(height - 1)?.checked_add(row_bytes)?;
    if raw.data.len() < described {
        return None;
    }

    let mut pixels = Vec::with_capacity(width.checked_mul(height)?.checked_mul(4)?);
    for row in 0..height {
        let source = &raw.data[row * stride..row * stride + row_bytes];
        for pixel in source.chunks_exact(channels) {
            pixels.extend_from_slice(&[
                pixel[0],
                pixel[1],
                pixel[2],
                if channels == 4 { pixel[3] } else { u8::MAX },
            ]);
        }
    }
    Some(Picture {
        width: raw.width as u32,
        height: raw.height as u32,
        pixels: Arc::from(pixels),
    })
}
