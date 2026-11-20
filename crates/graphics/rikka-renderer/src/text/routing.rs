use rikka_text::{ColorScheme, GlyphKey};

use crate::text::atlas::AtlasKey;

const CURVES_ABOVE_PX: f32 = 64.0;
const COLOR_RASTER_CAP_PX: f32 = 256.0;
const ZOOMED_COLOR_STEPS_PER_OCTAVE: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GlyphRouting {
    curves_above: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GlyphRoute {
    pub key: AtlasKey,
    pub scale: f32,
}

impl GlyphRoute {
    pub(crate) fn unscaled(key: AtlasKey) -> Self {
        Self { key, scale: 1.0 }
    }
}

impl Default for GlyphRouting {
    fn default() -> Self {
        Self {
            curves_above: CURVES_ABOVE_PX,
        }
    }
}

impl GlyphRouting {
    pub(crate) fn route(self, key: GlyphKey, scheme: ColorScheme, zooming: bool) -> GlyphRoute {
        if key.is_color() {
            let rastered = color_raster(key, zooming).with_color_scheme(scheme);
            return GlyphRoute {
                key: AtlasKey::Text(rastered),
                scale: key.size() / rastered.size(),
            };
        }
        GlyphRoute::unscaled(match zooming || key.size() > self.curves_above {
            true => AtlasKey::Curves(key.curve_key()),
            false => AtlasKey::Text(key),
        })
    }
}

fn color_raster(key: GlyphKey, zooming: bool) -> GlyphKey {
    let size = key.size();
    let target = match zooming {
        true => {
            let steps = (size.max(1.0).log2() * ZOOMED_COLOR_STEPS_PER_OCTAVE).ceil();
            (steps / ZOOMED_COLOR_STEPS_PER_OCTAVE).exp2()
        }
        false => size,
    }
    .min(COLOR_RASTER_CAP_PX);
    match target == size {
        true => key,
        false => key.resized(target),
    }
}
