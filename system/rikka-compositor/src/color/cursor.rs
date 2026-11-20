use std::collections::HashMap;

use rikka_colorimetry::{BlendSpace, Description, HdrSpace, Intent, Mapping};

pub(crate) fn encode_cursor(argb: &[u8], space: HdrSpace) -> Vec<u8> {
    let mapping = Mapping::new(
        &Description::SRGB,
        Intent::Perceptual,
        &BlendSpace::Hdr(space),
    )
    .expect("sRGB maps into the HDR blend space");
    let mut encoded: HashMap<[u8; 4], [u8; 4]> = HashMap::new();
    argb.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|pixel| {
            *encoded
                .entry(*pixel)
                .or_insert_with(|| encode_pixel(*pixel, &mapping, space))
        })
        .collect()
}

fn encode_pixel([blue, green, red, alpha]: [u8; 4], mapping: &Mapping, space: HdrSpace) -> [u8; 4] {
    if alpha == 0 {
        return [0; 4];
    }
    let coverage = f64::from(alpha) / 255.0;
    let straight =
        [red, green, blue].map(|channel| (f64::from(channel) / 255.0 / coverage).min(1.0));
    let [r, g, b] = space
        .encode(mapping.apply(straight))
        .map(|channel| (channel * coverage * 255.0).round().clamp(0.0, 255.0) as u8);
    [b, g, r, alpha]
}
