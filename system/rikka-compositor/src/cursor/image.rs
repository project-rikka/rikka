use std::sync::Arc;

use rikka_compositor_seat::CursorShape;
use rikka_xcursor::{Cursor, Theme};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CursorImage {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) hotspot: (i32, i32),
    rgba: Vec<u8>,
}

impl CursorImage {
    pub(crate) fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub(crate) fn argb(&self) -> Vec<u8> {
        swapped(&self.rgba)
    }
}

fn swapped(pixels: &[u8]) -> Vec<u8> {
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|[first, second, third, alpha]| [*third, *second, *first, *alpha])
        .collect()
}

pub(crate) fn load(theme: &Theme, shape: CursorShape, size: u32) -> Option<Arc<CursorImage>> {
    let path = std::iter::once(shape.name())
        .chain(shape.alt_names().iter().copied())
        .chain(std::iter::once(CursorShape::Default.name()))
        .chain(CursorShape::Default.alt_names().iter().copied())
        .find_map(|name| theme.find(name))?;
    let cursor = Cursor::read(&path, size)
        .inspect_err(|error| tracing::debug!("cannot read cursor {}: {error}", path.display()))
        .ok()?;
    let frame = cursor.frames.into_iter().next()?;
    Some(Arc::new(CursorImage {
        width: frame.width,
        height: frame.height,
        hotspot: (frame.hotspot.0.cast_signed(), frame.hotspot.1.cast_signed()),
        rgba: swapped(&frame.bgra),
    }))
}

const FALLBACK_ARROW: [&str; 19] = [
    "X           ",
    "XX          ",
    "X.X         ",
    "X..X        ",
    "X...X       ",
    "X....X      ",
    "X.....X     ",
    "X......X    ",
    "X.......X   ",
    "X........X  ",
    "X.....XXXXX ",
    "X..X..X     ",
    "X.X X..X    ",
    "XX  X..X    ",
    "X    X..X   ",
    "     X..X   ",
    "      X..X  ",
    "      X..X  ",
    "       XX   ",
];

pub(crate) fn fallback() -> Arc<CursorImage> {
    let height = FALLBACK_ARROW.len() as u32;
    let width = FALLBACK_ARROW[0].len() as u32;
    let rgba = FALLBACK_ARROW
        .iter()
        .flat_map(|row| row.bytes())
        .flat_map(|cell| match cell {
            b'X' => [0, 0, 0, 255],
            b'.' => [255, 255, 255, 255],
            _ => [0, 0, 0, 0],
        })
        .collect();
    Arc::new(CursorImage {
        width,
        height,
        hotspot: (0, 0),
        rgba,
    })
}
