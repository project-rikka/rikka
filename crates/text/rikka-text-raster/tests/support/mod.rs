pub(crate) mod colr;
pub(crate) mod sfnt;

use rikka_text_font::{Font, GlyphId};
use rikka_text_raster::GlyphImage;

pub(crate) fn glyph(font: &Font<'_>, character: char) -> GlyphId {
    font.charmap()
        .and_then(|charmap| charmap.map(u32::from(character)))
        .unwrap_or_else(|| panic!("{character:?} is not mapped"))
}

pub(crate) fn ink(image: &GlyphImage) -> f64 {
    image
        .pixels()
        .iter()
        .map(|&value| f64::from(value))
        .sum::<f64>()
        / 255.0
}

pub(crate) fn pixel(image: &GlyphImage, x: u32, y: u32) -> &[u8] {
    let bytes = image.kind().bytes_per_pixel();
    let row = image.row(y);
    &row[x as usize * bytes..(x as usize + 1) * bytes]
}
