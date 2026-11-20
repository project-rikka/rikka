use std::sync::LazyLock;

use rikka_types::srgb;

pub(crate) type Rgba = [f32; 4];

const ENCODE_STEPS: usize = 4096;

static DECODE: LazyLock<[f32; 256]> =
    LazyLock::new(|| std::array::from_fn(|index| srgb::to_linear(index as f32 / 255.0)));

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
pub(crate) fn linear(channel: u8) -> f32 {
    DECODE[usize::from(channel)]
}

pub(crate) struct Encoder {
    table: &'static [f32],
}

impl Encoder {
    pub(crate) fn new() -> Self {
        Self { table: &ENCODE }
    }

    pub(crate) fn write_linear_premultiplied(&self, source: &[Rgba], out: &mut [[u8; 4]]) {
        for (target, pixel) in out.iter_mut().zip(source) {
            let alpha = pixel[3].clamp(0.0, 1.0);
            if alpha <= 0.0 {
                *target = [0; 4];
                continue;
            }
            let alpha8 = u32::from(to_byte(alpha));
            let inverse = 1.0 / alpha;
            let channel = |value: f32| -> u8 {
                let encoded = u32::from(to_byte(encode_unit(self.table, value * inverse)));
                ((encoded * alpha8 + 127) / 255) as u8
            };
            *target = [
                channel(pixel[0]),
                channel(pixel[1]),
                channel(pixel[2]),
                alpha8 as u8,
            ];
        }
    }
}

pub(crate) fn straight_rgba8_to_linear(pixel: [u8; 4]) -> Rgba {
    let alpha = f32::from(pixel[3]) / 255.0;
    [
        linear(pixel[0]) * alpha,
        linear(pixel[1]) * alpha,
        linear(pixel[2]) * alpha,
        alpha,
    ]
}
