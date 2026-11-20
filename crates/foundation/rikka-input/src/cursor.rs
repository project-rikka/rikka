use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

use rikka_sync::Mutex;
use rikka_types::CursorImage;

#[derive(Clone, PartialEq, Eq)]
pub struct CursorPicture {
    pub width: u32,
    pub height: u32,
    pub hotspot_x: i32,
    pub hotspot_y: i32,
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for CursorPicture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CursorPicture")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("hotspot_x", &self.hotspot_x)
            .field("hotspot_y", &self.hotspot_y)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CursorPixels {
    pub width: u32,
    pub height: u32,
    pub hotspot: (i32, i32),
    pub premultiplied_bgra: Vec<u8>,
}

impl std::fmt::Debug for CursorPixels {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CursorPixels")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("hotspot", &self.hotspot)
            .finish_non_exhaustive()
    }
}

static PICTURES: LazyLock<Mutex<HashMap<u64, Arc<CursorPixels>>>> = LazyLock::new(Mutex::default);
static NEXT: AtomicU64 = AtomicU64::new(1);

pub fn define_cursor(picture: CursorPicture) -> Option<CursorImage> {
    let length = (picture.width as usize)
        .checked_mul(picture.height as usize)?
        .checked_mul(4)?;
    if length == 0 || picture.rgba.len() != length {
        return None;
    }
    let premultiplied_bgra = picture
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[red, green, blue, opacity]| {
            let scale = |channel: u8| ((u16::from(channel) * u16::from(opacity) + 127) / 255) as u8;
            [scale(blue), scale(green), scale(red), opacity]
        })
        .collect();
    let pixels = CursorPixels {
        width: picture.width,
        height: picture.height,
        hotspot: (picture.hotspot_x, picture.hotspot_y),
        premultiplied_bgra,
    };
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    PICTURES.lock().insert(id, Arc::new(pixels));
    Some(CursorImage::new(id))
}

pub fn forget_cursor(image: CursorImage) {
    PICTURES.lock().remove(&image.id());
}

#[must_use]
pub fn cursor_pixels(image: CursorImage) -> Option<Arc<CursorPixels>> {
    PICTURES.lock().get(&image.id()).cloned()
}
