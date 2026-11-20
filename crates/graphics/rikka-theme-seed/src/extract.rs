use rikka_material_color::{Rgb, quantize, score};
use rikka_types::Color;

const CLUSTERS: usize = 128;
const OPAQUE: u8 = 255;

pub(crate) fn source_color(rgba: &[u8]) -> Option<Color> {
    let pixels = rgba.as_chunks::<4>().0;
    if pixels.is_empty() {
        return None;
    }
    let colors = |opaque_only: bool| -> Vec<Rgb> {
        pixels
            .iter()
            .filter(|pixel| !opaque_only || pixel[3] == OPAQUE)
            .map(|pixel| Rgb::new(pixel[0], pixel[1], pixel[2]))
            .collect()
    };
    let opaque = colors(true);
    let counted = match opaque.is_empty() {
        true => quantize(&colors(false), CLUSTERS),
        false => quantize(&opaque, CLUSTERS),
    };
    score(&counted)
        .first()
        .map(|rgb| Color::rgb8(rgb.red, rgb.green, rgb.blue))
}
