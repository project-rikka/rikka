use rikka_material_color::{Hct, Rgb, TonalPalette};
use rikka_types::Color;

pub(crate) fn with_tone(color: Color, tone: f32) -> Color {
    let tone = f64::from(tone.clamp(0.0, 100.0).round());
    from_rgb(TonalPalette::from_hct(Hct::of(rgb(color))).tone(tone)).with_alpha(color.a)
}

pub(crate) fn muted(color: Color, tone: f32, chroma: f64) -> Color {
    shifted(color, tone, |own| own.min(chroma))
}

pub(crate) fn shifted(color: Color, tone: f32, chroma: impl FnOnce(f64) -> f64) -> Color {
    let hct = Hct::of(rgb(color));
    let adjusted = Hct::solve(
        hct.hue(),
        chroma(hct.chroma()).max(0.0),
        f64::from(tone.clamp(0.0, 100.0)),
    );
    from_rgb(adjusted.rgb()).with_alpha(color.a)
}

pub(crate) fn measure(color: Color) -> f32 {
    rgb(color).lstar() as f32
}

pub(crate) fn rgb(color: Color) -> Rgb {
    let [red, green, blue, _] = color.to_rgba8();
    Rgb::new(red, green, blue)
}

pub(crate) fn from_rgb(rgb: Rgb) -> Color {
    Color::rgb8(rgb.red, rgb.green, rgb.blue)
}
