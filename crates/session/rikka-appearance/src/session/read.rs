use rikka_settings::Store;
use rikka_settings::session::appearance;
use rikka_theme::{MaterialStyle, MaterialYou, Translucency};
use rikka_types::Color;

use crate::dress;

pub(crate) fn material(store: &Store) -> MaterialYou {
    let text = |key: &str| store.get::<String>(key).unwrap_or_default();
    let contrast = store
        .get::<f64>(appearance::MATERIAL_CONTRAST)
        .unwrap_or(appearance::MATERIAL_CONTRAST_DEFAULT);
    let generated = match source(&text(appearance::MATERIAL_SOURCE)) {
        Some(source) => MaterialYou::new(source),
        None => MaterialYou::default(),
    };
    generated
        .style(style(&text(appearance::MATERIAL_STYLE)))
        .contrast(contrast as f32)
}

pub(crate) fn glaze(store: &Store) -> Translucency {
    let number = |key: &str, fallback: f64| store.get::<f64>(key).unwrap_or(fallback);
    let blurs = store
        .get::<bool>(appearance::BLUR)
        .unwrap_or(appearance::BLUR_DEFAULT);
    dress::glaze(
        number(appearance::OPACITY, appearance::OPACITY_DEFAULT),
        blurs,
        number(appearance::BLUR_RADIUS, appearance::BLUR_RADIUS_DEFAULT),
    )
}

fn source(setting: &str) -> Option<Color> {
    if setting == appearance::UNSET {
        return None;
    }
    let parsed = Color::parse_hex(setting);
    if parsed.is_none() {
        tracing::warn!("`{setting}` is not a color, using built-in seed color");
    }
    parsed
}

fn style(setting: &str) -> MaterialStyle {
    MaterialStyle::from_slug(setting).unwrap_or_else(|| {
        if !setting.is_empty() && setting != appearance::MATERIAL_STYLE_DEFAULT {
            tracing::warn!("unknown Material You scheme `{setting}`, using default");
        }
        MaterialStyle::default()
    })
}
