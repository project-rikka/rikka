use std::path::PathBuf;
use std::sync::Arc;

use zbus::zvariant::{OwnedValue, Type, Value};

use rikka_bus::Picture;

#[derive(Clone, Debug, PartialEq)]
pub enum Icon {
    Named {
        name: String,
        theme_path: Option<PathBuf>,
    },
    Pixels(Picture),
    Bytes(Arc<[u8]>),
}

#[derive(Clone, Debug, Type, Value, OwnedValue)]
pub(crate) struct RawPixmap {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) data: Vec<u8>,
}

const MOST: i32 = 512;

const WANTED: i32 = 48;

impl Icon {
    #[must_use]
    pub(crate) fn best(
        name: &str,
        pixmaps: &[RawPixmap],
        theme_path: Option<&str>,
    ) -> Option<Icon> {
        pixels(pixmaps).map(Icon::Pixels).or_else(|| {
            let name = name.trim();
            (!name.is_empty()).then(|| Icon::Named {
                name: name.to_owned(),
                theme_path: theme_path
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
                    .map(PathBuf::from),
            })
        })
    }
}

#[must_use]
pub(crate) fn pixels(pixmaps: &[RawPixmap]) -> Option<Picture> {
    let mut sound: Vec<&RawPixmap> = pixmaps.iter().filter(|raw| describes(raw)).collect();
    sound.sort_by_key(|raw| raw.width.min(raw.height));
    let selected = sound
        .iter()
        .copied()
        .find(|raw| raw.width.min(raw.height) >= WANTED)
        .or_else(|| sound.last().copied())?;
    Some(straighten(selected))
}

fn describes(raw: &RawPixmap) -> bool {
    if !(1..=MOST).contains(&raw.width) || !(1..=MOST).contains(&raw.height) {
        return false;
    }
    (raw.width as usize)
        .checked_mul(raw.height as usize)
        .and_then(|count| count.checked_mul(4))
        .is_some_and(|described| raw.data.len() >= described)
}

fn straighten(raw: &RawPixmap) -> Picture {
    let described = (raw.width as usize) * (raw.height as usize) * 4;
    let mut pixels = Vec::with_capacity(described);
    for pixel in raw.data[..described].as_chunks::<4>().0 {
        pixels.extend_from_slice(&[pixel[1], pixel[2], pixel[3], pixel[0]]);
    }
    Picture {
        width: raw.width as u32,
        height: raw.height as u32,
        pixels: Arc::from(pixels),
    }
}
