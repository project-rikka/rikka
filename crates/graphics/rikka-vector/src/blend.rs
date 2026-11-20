use crate::color::Rgba;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlendMode {
    Clear,
    Source,
    Destination,
    #[default]
    SourceOver,
    DestinationOver,
    SourceIn,
    DestinationIn,
    SourceOut,
    DestinationOut,
    SourceAtop,
    DestinationAtop,
    Xor,
    Plus,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Multiply,
    HslHue,
    HslSaturation,
    HslColor,
    HslLuminosity,
}

#[inline]
pub(crate) fn source_over(source: Rgba, backdrop: Rgba) -> Rgba {
    let keep = 1.0 - source[3];
    [
        source[0] + backdrop[0] * keep,
        source[1] + backdrop[1] * keep,
        source[2] + backdrop[2] * keep,
        source[3] + backdrop[3] * keep,
    ]
}

impl BlendMode {
    #[must_use]
    pub fn composite(self, source: [f32; 4], backdrop: [f32; 4]) -> [f32; 4] {
        composite(self, source, backdrop)
    }
}

pub(crate) fn composite(mode: BlendMode, source: Rgba, backdrop: Rgba) -> Rgba {
    let (sa, ba) = (source[3], backdrop[3]);
    let porter_duff = |fa: f32, fb: f32| -> Rgba {
        std::array::from_fn(|channel| source[channel] * fa + backdrop[channel] * fb)
    };
    match mode {
        BlendMode::Clear => [0.0; 4],
        BlendMode::Source => source,
        BlendMode::Destination => backdrop,
        BlendMode::SourceOver => source_over(source, backdrop),
        BlendMode::DestinationOver => porter_duff(1.0 - ba, 1.0),
        BlendMode::SourceIn => porter_duff(ba, 0.0),
        BlendMode::DestinationIn => porter_duff(0.0, sa),
        BlendMode::SourceOut => porter_duff(1.0 - ba, 0.0),
        BlendMode::DestinationOut => porter_duff(0.0, 1.0 - sa),
        BlendMode::SourceAtop => porter_duff(ba, 1.0 - sa),
        BlendMode::DestinationAtop => porter_duff(1.0 - ba, sa),
        BlendMode::Xor => porter_duff(1.0 - ba, 1.0 - sa),
        BlendMode::Plus => {
            std::array::from_fn(|channel| (source[channel] + backdrop[channel]).min(1.0))
        }
        BlendMode::Screen
        | BlendMode::Overlay
        | BlendMode::Darken
        | BlendMode::Lighten
        | BlendMode::ColorDodge
        | BlendMode::ColorBurn
        | BlendMode::HardLight
        | BlendMode::SoftLight
        | BlendMode::Difference
        | BlendMode::Exclusion
        | BlendMode::Multiply => {
            let blend = separable(mode);
            blended(source, backdrop, |cs, cb| {
                [
                    blend(cb[0], cs[0]),
                    blend(cb[1], cs[1]),
                    blend(cb[2], cs[2]),
                ]
            })
        }
        BlendMode::HslHue => blended(source, backdrop, |cs, cb| {
            set_luminosity(set_saturation(cs, saturation(cb)), luminosity(cb))
        }),
        BlendMode::HslSaturation => blended(source, backdrop, |cs, cb| {
            set_luminosity(set_saturation(cb, saturation(cs)), luminosity(cb))
        }),
        BlendMode::HslColor => blended(source, backdrop, |cs, cb| {
            set_luminosity(cs, luminosity(cb))
        }),
        BlendMode::HslLuminosity => blended(source, backdrop, |cs, cb| {
            set_luminosity(cb, luminosity(cs))
        }),
    }
}

fn blended(source: Rgba, backdrop: Rgba, blend: impl Fn([f32; 3], [f32; 3]) -> [f32; 3]) -> Rgba {
    let (sa, ba) = (source[3], backdrop[3]);
    let straight = |pixel: Rgba, alpha: f32| -> [f32; 3] {
        if alpha > 0.0 {
            std::array::from_fn(|channel| (pixel[channel] / alpha).clamp(0.0, 1.0))
        } else {
            [0.0; 3]
        }
    };
    let mixed = blend(straight(source, sa), straight(backdrop, ba));
    let both = sa * ba;
    let mut result = [0.0; 4];
    for channel in 0..3 {
        result[channel] =
            source[channel] * (1.0 - ba) + backdrop[channel] * (1.0 - sa) + both * mixed[channel];
    }
    result[3] = sa + ba - both;
    result
}

fn separable(mode: BlendMode) -> fn(f32, f32) -> f32 {
    match mode {
        BlendMode::Multiply => |cb, cs| cb * cs,
        BlendMode::Screen => screen,
        BlendMode::Overlay => |cb, cs| hard_light(cs, cb),
        BlendMode::Darken => f32::min,
        BlendMode::Lighten => f32::max,
        BlendMode::ColorDodge => |cb, cs| {
            if cb <= 0.0 {
                0.0
            } else if cs >= 1.0 {
                1.0
            } else {
                (cb / (1.0 - cs)).min(1.0)
            }
        },
        BlendMode::ColorBurn => |cb, cs| {
            if cb >= 1.0 {
                1.0
            } else if cs <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - cb) / cs).min(1.0)
            }
        },
        BlendMode::HardLight => hard_light,
        BlendMode::SoftLight => soft_light,
        BlendMode::Difference => |cb, cs| (cb - cs).abs(),
        _ => |cb, cs| cb + cs - 2.0 * cb * cs,
    }
}

fn screen(cb: f32, cs: f32) -> f32 {
    cb + cs - cb * cs
}

fn hard_light(cb: f32, cs: f32) -> f32 {
    if cs <= 0.5 {
        cb * 2.0 * cs
    } else {
        screen(cb, 2.0 * cs - 1.0)
    }
}

fn soft_light(cb: f32, cs: f32) -> f32 {
    if cs <= 0.5 {
        cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
    } else {
        let d = if cb <= 0.25 {
            ((16.0 * cb - 12.0) * cb + 4.0) * cb
        } else {
            cb.sqrt()
        };
        cb + (2.0 * cs - 1.0) * (d - cb)
    }
}

fn luminosity(color: [f32; 3]) -> f32 {
    0.3 * color[0] + 0.59 * color[1] + 0.11 * color[2]
}

fn clip_color(color: [f32; 3]) -> [f32; 3] {
    let l = luminosity(color);
    let n = color[0].min(color[1]).min(color[2]);
    let x = color[0].max(color[1]).max(color[2]);
    let mut result = color;
    if n < 0.0 && l - n > 0.0 {
        result = result.map(|c| l + (c - l) * l / (l - n));
    }
    if x > 1.0 && x - l > 0.0 {
        result = result.map(|c| l + (c - l) * (1.0 - l) / (x - l));
    }
    result
}

fn set_luminosity(color: [f32; 3], target: f32) -> [f32; 3] {
    let delta = target - luminosity(color);
    clip_color(color.map(|c| c + delta))
}

fn saturation(color: [f32; 3]) -> f32 {
    color[0].max(color[1]).max(color[2]) - color[0].min(color[1]).min(color[2])
}

fn set_saturation(color: [f32; 3], target: f32) -> [f32; 3] {
    let max = color[0].max(color[1]).max(color[2]);
    let min = color[0].min(color[1]).min(color[2]);
    if max <= min {
        return [0.0; 3];
    }
    color.map(|c| (c - min) * target / (max - min))
}
