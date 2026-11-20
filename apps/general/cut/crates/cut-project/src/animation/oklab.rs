use rikka_types::srgb::{from_linear_f64, to_linear_f64};
use rikka_types::{Color, oklab};

pub(crate) fn from_color(color: Color) -> [f64; 4] {
    let linear = [color.r, color.g, color.b].map(|channel| to_linear_f64(f64::from(channel)));
    let [lightness, a, b] = oklab::from_linear(linear);
    [lightness, a, b, f64::from(color.a)]
}

pub(crate) fn to_color([lightness, a, b, alpha]: [f64; 4]) -> Color {
    let [red, green, blue] = oklab::to_linear([lightness, a, b])
        .map(|channel| from_linear_f64(channel.clamp(0.0, 1.0)) as f32);
    Color::rgba(red, green, blue, alpha.clamp(0.0, 1.0) as f32)
}
