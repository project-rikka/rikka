use rikka_theme::Palette;
use rikka_types::Color;

const FLOOR: f32 = 0.004;
const LIGHTEST: f32 = 0.10;
const DARKEST: f32 = 0.50;

pub(crate) fn shade(palette: &Palette, heat: f32) -> Option<Color> {
    let heat = heat.clamp(0.0, 1.0);
    if heat < FLOOR {
        return None;
    }
    let along = heat.powf(0.45);
    let colour = if along < 0.5 {
        palette.accent.mix(palette.warning, along * 2.0)
    } else {
        palette.warning.mix(palette.danger, (along - 0.5) * 2.0)
    };
    Some(colour.with_alpha(LIGHTEST + (DARKEST - LIGHTEST) * along))
}
