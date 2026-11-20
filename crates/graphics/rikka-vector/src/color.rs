use std::sync::LazyLock;

use rikka_types::{Color, srgb};

pub(crate) type Rgba = [f32; 4];

pub(crate) const TRANSPARENT: Rgba = [0.0; 4];

const ENCODE_STEPS: usize = 4096;

static ENCODE: LazyLock<Box<[f32]>> = LazyLock::new(|| {
    (0..=ENCODE_STEPS)
        .map(|step| srgb::from_linear(step as f32 / ENCODE_STEPS as f32))
        .collect()
});

#[inline]
fn encode_unit(table: &[f32], linear: f32) -> f32 {
    let position = linear.clamp(0.0, 1.0) * ENCODE_STEPS as f32;
    let index = (position as usize).min(ENCODE_STEPS - 1);
    let fraction = position - index as f32;
    table[index] + (table[index + 1] - table[index]) * fraction
}

#[inline]
pub(crate) fn to_byte(unit: f32) -> u8 {
    (unit.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

#[inline]
pub(crate) fn straight(color: Color) -> Rgba {
    [
        color.r.clamp(0.0, 1.0),
        color.g.clamp(0.0, 1.0),
        color.b.clamp(0.0, 1.0),
        color.a.clamp(0.0, 1.0),
    ]
}

#[inline]
pub(crate) fn premultiplied(color: Color) -> Rgba {
    let [r, g, b, a] = straight(color);
    [r * a, g * a, b * a, a]
}

pub(crate) fn linear_premultiplied(color: Color) -> Rgba {
    let [r, g, b, a] = straight(color);
    [
        srgb::to_linear(r) * a,
        srgb::to_linear(g) * a,
        srgb::to_linear(b) * a,
        a,
    ]
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Encoder {
    table: &'static [f32],
}

impl Encoder {
    pub(crate) fn new() -> Self {
        Self { table: &ENCODE }
    }

    #[inline]
    pub(crate) fn srgb_premultiplied(&self, pixel: Rgba) -> Rgba {
        let alpha = pixel[3].clamp(0.0, 1.0);
        if alpha <= 0.0 {
            return TRANSPARENT;
        }
        let inverse = 1.0 / alpha;
        let channel = |value: f32| encode_unit(self.table, value * inverse) * alpha;
        [
            channel(pixel[0]),
            channel(pixel[1]),
            channel(pixel[2]),
            alpha,
        ]
    }
}

#[inline]
pub(crate) fn load(pixel: [u8; 4]) -> Rgba {
    const UNIT: f32 = 1.0 / 255.0;
    [
        f32::from(pixel[0]) * UNIT,
        f32::from(pixel[1]) * UNIT,
        f32::from(pixel[2]) * UNIT,
        f32::from(pixel[3]) * UNIT,
    ]
}

#[inline]
pub(crate) fn store(pixel: Rgba) -> [u8; 4] {
    let alpha = pixel[3].clamp(0.0, 1.0);
    [
        to_byte(pixel[0].min(alpha)),
        to_byte(pixel[1].min(alpha)),
        to_byte(pixel[2].min(alpha)),
        to_byte(alpha),
    ]
}
