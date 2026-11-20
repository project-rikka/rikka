mod adjusted;
mod separable;

use rikka_psd::BlendMode;

pub(crate) use crate::blend::adjusted::blend_adjusted;
pub(crate) use crate::blend::separable::Blend;
use crate::blend::separable::{
    ColorBurn, ColorDodge, Darken, Difference, Divide, Exclusion, HardLight, HardMix, Lighten,
    LinearBurn, LinearDodge, LinearLight, Multiply, Normal, Overlay, PinLight, Screen, SoftLight,
    Subtract, VividLight,
};
use crate::space::{Space, SpaceKind};

pub(crate) trait Blending {
    type Output;

    fn separable<B: Blend>(self) -> Self::Output;

    fn per_pixel(self) -> Self::Output;
}

pub(crate) fn blending<W: Blending>(mode: BlendMode, work: W) -> W::Output {
    match mode {
        BlendMode::Normal | BlendMode::PassThrough | BlendMode::Unknown(_) => {
            work.separable::<Normal>()
        }
        BlendMode::Multiply => work.separable::<Multiply>(),
        BlendMode::Screen => work.separable::<Screen>(),
        BlendMode::Overlay => work.separable::<Overlay>(),
        BlendMode::Darken => work.separable::<Darken>(),
        BlendMode::Lighten => work.separable::<Lighten>(),
        BlendMode::ColorDodge => work.separable::<ColorDodge>(),
        BlendMode::ColorBurn => work.separable::<ColorBurn>(),
        BlendMode::LinearDodge => work.separable::<LinearDodge>(),
        BlendMode::LinearBurn => work.separable::<LinearBurn>(),
        BlendMode::HardLight => work.separable::<HardLight>(),
        BlendMode::SoftLight => work.separable::<SoftLight>(),
        BlendMode::VividLight => work.separable::<VividLight>(),
        BlendMode::LinearLight => work.separable::<LinearLight>(),
        BlendMode::PinLight => work.separable::<PinLight>(),
        BlendMode::HardMix => work.separable::<HardMix>(),
        BlendMode::Difference => work.separable::<Difference>(),
        BlendMode::Exclusion => work.separable::<Exclusion>(),
        BlendMode::Subtract => work.separable::<Subtract>(),
        BlendMode::Divide => work.separable::<Divide>(),
        BlendMode::Dissolve
        | BlendMode::DarkerColor
        | BlendMode::LighterColor
        | BlendMode::Hue
        | BlendMode::Saturation
        | BlendMode::Color
        | BlendMode::Luminosity => work.per_pixel(),
    }
}
pub(crate) fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn clip_color(c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut out = c;
    if n < 0.0 {
        let denominator = (l - n).max(1e-9);
        out = out.map(|value| l + (value - l) * l / denominator);
    }
    if x > 1.0 {
        let denominator = (x - l).max(1e-9);
        out = out.map(|value| l + (value - l) * (1.0 - l) / denominator);
    }
    out.map(|value| value.clamp(0.0, 1.0))
}

pub(crate) fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color(c.map(|value| value + d))
}

fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let mut order = [0usize, 1, 2];
    order.sort_by(|&a, &b| c[a].total_cmp(&c[b]));
    let (min, mid, max) = (order[0], order[1], order[2]);
    let mut out = [0.0f32; 3];
    if c[max] > c[min] {
        out[mid] = (c[mid] - c[min]) * s / (c[max] - c[min]);
        out[max] = s;
    }
    out
}

fn nonseparable(mode: BlendMode, cb: [f32; 3], cs: [f32; 3]) -> [f32; 3] {
    match mode {
        BlendMode::Hue => set_lum(set_sat(cs, sat(cb)), lum(cb)),
        BlendMode::Saturation => set_lum(set_sat(cb, sat(cs)), lum(cb)),
        BlendMode::Color => set_lum(cs, lum(cb)),
        BlendMode::Luminosity => set_lum(cb, lum(cs)),
        _ => cs,
    }
}

pub(crate) fn blend(mode: BlendMode, space: &Space, cb: &[f32], cs: &[f32], out: &mut [f32]) {
    let channels = space.channels();
    match mode {
        BlendMode::DarkerColor | BlendMode::LighterColor => {
            let lightness = |pixel: &[f32]| match space.kind {
                SpaceKind::Cmyk => lum([
                    pixel[0] * pixel[3],
                    pixel[1] * pixel[3],
                    pixel[2] * pixel[3],
                ]),
                SpaceKind::Gray => pixel[0],
                _ => lum([pixel[0], pixel[1], pixel[2]]),
            };
            let source_darker = lightness(cs) < lightness(cb);
            let pick_source = if mode == BlendMode::DarkerColor {
                source_darker
            } else {
                lightness(cs) > lightness(cb)
            };
            out[..channels].copy_from_slice(if pick_source {
                &cs[..channels]
            } else {
                &cb[..channels]
            });
        }
        BlendMode::Hue | BlendMode::Saturation | BlendMode::Color | BlendMode::Luminosity => {
            if channels < 3 {
                out[..channels].copy_from_slice(&cs[..channels]);
                return;
            }
            let result = nonseparable(mode, [cb[0], cb[1], cb[2]], [cs[0], cs[1], cs[2]]);
            out[..3].copy_from_slice(&result);
            if channels == 4 {
                out[3] = if mode == BlendMode::Luminosity {
                    cs[3]
                } else {
                    cb[3]
                };
            }
        }
        _ => out[..channels].copy_from_slice(&cs[..channels]),
    }
}

pub(crate) fn dissolve_visible(x: i32, y: i32, alpha: f32) -> bool {
    let mut hash = (x as u32).wrapping_mul(0x9e37_79b1) ^ (y as u32).wrapping_mul(0x85eb_ca77);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x2c1b_3c6d);
    hash ^= hash >> 12;
    (hash as f32 / u32::MAX as f32) < alpha
}
