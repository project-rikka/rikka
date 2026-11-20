mod wsmeans;
mod wu;

use crate::color::Rgb;

#[must_use]
pub fn quantize(pixels: &[Rgb], max_colors: usize) -> Vec<(Rgb, u32)> {
    wsmeans::refine(pixels, max_colors, &wu::boxes(pixels, max_colors))
}
