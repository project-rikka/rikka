use rikka_settings::session::appearance::Background;
use rikka_theme::{ThemeSpec, Translucency};

pub(crate) fn dressed(spec: ThemeSpec, background: Background) -> ThemeSpec {
    match background {
        Background::Solid => spec,
        Background::Glass => spec.glass(),
        Background::Liquid => spec.liquid(),
    }
}

pub(crate) fn glaze(opacity: f64, blurs: bool, radius: f64) -> Translucency {
    let radius = match blurs {
        true => radius,
        false => 0.0,
    };
    Translucency::new(opacity as f32).with_blur(radius as f32)
}
