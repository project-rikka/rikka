use rikka_types::{Color, srgb};

use crate::color_type::ColorType;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Pixels {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Vec<f32>),
}

impl Pixels {
    pub(crate) fn as_bytes(&self) -> &[u8] {
        match self {
            Pixels::U8(samples) => samples,
            Pixels::U16(samples) => bytemuck::cast_slice(samples),
            Pixels::F32(samples) => bytemuck::cast_slice(samples),
        }
    }

    pub(crate) fn as_bytes_mut(&mut self) -> &mut [u8] {
        match self {
            Pixels::U8(samples) => samples,
            Pixels::U16(samples) => bytemuck::cast_slice_mut(samples),
            Pixels::F32(samples) => bytemuck::cast_slice_mut(samples),
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Pixels::U8(samples) => samples.len(),
            Pixels::U16(samples) => samples.len(),
            Pixels::F32(samples) => samples.len(),
        }
    }

    pub(crate) fn zeroed(color: ColorType, samples: usize) -> Pixels {
        match color.bits_per_channel() {
            8 => Pixels::U8(vec![0; samples]),
            16 => Pixels::U16(vec![0; samples]),
            _ => Pixels::F32(vec![0.0; samples]),
        }
    }
}

pub(crate) trait Sample: Copy + PartialEq + Default + Send + Sync + bytemuck::Pod {
    const FULL: Self;

    fn to_unit(self) -> f32;
    fn from_unit(value: f32) -> Self;
    fn scaled(self) -> f32;
    fn from_scaled(value: f32) -> Self;
    fn luma(rgb: [Self; 3]) -> Self;
    fn from_u8(value: u8) -> Self;
    fn from_u16(value: u16) -> Self;
    fn from_f32(value: f32) -> Self;
    fn cast<D: Sample>(self) -> D;
    fn wrap(samples: Vec<Self>) -> Pixels;
}

const LUMA_WEIGHTS: [u32; 3] = [2126, 7152, 722];
const LUMA_SCALE: u32 = 10000;

impl Sample for u8 {
    const FULL: u8 = u8::MAX;

    #[inline]
    fn to_unit(self) -> f32 {
        f32::from(self) / 255.0
    }

    #[inline]
    fn from_unit(value: f32) -> u8 {
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    #[inline]
    fn scaled(self) -> f32 {
        f32::from(self)
    }

    #[inline]
    fn from_scaled(value: f32) -> u8 {
        (value.max(0.0) + 0.5) as u8
    }

    #[inline]
    fn luma(rgb: [u8; 3]) -> u8 {
        let sum: u32 = (0..3).map(|at| LUMA_WEIGHTS[at] * u32::from(rgb[at])).sum();
        (sum / LUMA_SCALE) as u8
    }

    #[inline]
    fn from_u8(value: u8) -> u8 {
        value
    }

    #[inline]
    fn from_u16(value: u16) -> u8 {
        ((u32::from(value) + 128) / 257) as u8
    }

    #[inline]
    fn from_f32(value: f32) -> u8 {
        u8::from_unit(value)
    }

    #[inline]
    fn cast<D: Sample>(self) -> D {
        D::from_u8(self)
    }

    fn wrap(samples: Vec<u8>) -> Pixels {
        Pixels::U8(samples)
    }
}

impl Sample for u16 {
    const FULL: u16 = u16::MAX;

    #[inline]
    fn to_unit(self) -> f32 {
        f32::from(self) / 65535.0
    }

    #[inline]
    fn from_unit(value: f32) -> u16 {
        (value.clamp(0.0, 1.0) * 65535.0).round() as u16
    }

    #[inline]
    fn scaled(self) -> f32 {
        f32::from(self)
    }

    #[inline]
    fn from_scaled(value: f32) -> u16 {
        (value.max(0.0) + 0.5) as u16
    }

    #[inline]
    fn luma(rgb: [u16; 3]) -> u16 {
        let sum: u32 = (0..3).map(|at| LUMA_WEIGHTS[at] * u32::from(rgb[at])).sum();
        (sum / LUMA_SCALE) as u16
    }

    #[inline]
    fn from_u8(value: u8) -> u16 {
        u16::from(value) * 257
    }

    #[inline]
    fn from_u16(value: u16) -> u16 {
        value
    }

    #[inline]
    fn from_f32(value: f32) -> u16 {
        u16::from_unit(value)
    }

    #[inline]
    fn cast<D: Sample>(self) -> D {
        D::from_u16(self)
    }

    fn wrap(samples: Vec<u16>) -> Pixels {
        Pixels::U16(samples)
    }
}

impl Sample for f32 {
    const FULL: f32 = 1.0;

    #[inline]
    fn to_unit(self) -> f32 {
        self
    }

    #[inline]
    fn from_unit(value: f32) -> f32 {
        value
    }

    #[inline]
    fn scaled(self) -> f32 {
        self
    }

    #[inline]
    fn from_scaled(value: f32) -> f32 {
        value
    }

    #[inline]
    fn luma(rgb: [f32; 3]) -> f32 {
        0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
    }

    #[inline]
    fn from_u8(value: u8) -> f32 {
        value.to_unit()
    }

    #[inline]
    fn from_u16(value: u16) -> f32 {
        value.to_unit()
    }

    #[inline]
    fn from_f32(value: f32) -> f32 {
        value
    }

    #[inline]
    fn cast<D: Sample>(self) -> D {
        D::from_f32(self)
    }

    fn wrap(samples: Vec<f32>) -> Pixels {
        Pixels::F32(samples)
    }
}

macro_rules! by_depth {
    ($pixels:expr, $samples:ident => $body:expr) => {
        match $pixels {
            $crate::pixels::Pixels::U8($samples) => $body,
            $crate::pixels::Pixels::U16($samples) => $body,
            $crate::pixels::Pixels::F32($samples) => $body,
        }
    };
}

pub(crate) use by_depth;

pub(crate) fn fill(color_type: ColorType, color: Color, pixel_count: usize) -> Pixels {
    fn repeated<S: Sample>(color_type: ColorType, color: Color, pixel_count: usize) -> Vec<S> {
        let luma = srgb::luminance([color.r, color.g, color.b]);
        let pixel: [S; 4] = match (color_type.has_color(), color_type.has_alpha()) {
            (false, _) => [S::from_unit(luma), S::from_unit(color.a), S::FULL, S::FULL],
            (true, _) => [
                S::from_unit(color.r),
                S::from_unit(color.g),
                S::from_unit(color.b),
                S::from_unit(color.a),
            ],
        };
        let channels = usize::from(color_type.channel_count());
        let mut samples = Vec::with_capacity(pixel_count * channels);
        for _ in 0..pixel_count {
            samples.extend_from_slice(&pixel[..channels]);
        }
        samples
    }
    match color_type.bits_per_channel() {
        8 => Pixels::U8(repeated(color_type, color, pixel_count)),
        16 => Pixels::U16(repeated(color_type, color, pixel_count)),
        _ => Pixels::F32(repeated(color_type, color, pixel_count)),
    }
}

pub(crate) fn color_at(pixels: &Pixels, color_type: ColorType, index: usize) -> Color {
    fn read<S: Sample>(samples: &[S], color_type: ColorType, index: usize) -> Color {
        let channels = usize::from(color_type.channel_count());
        let pixel = &samples[index * channels..(index + 1) * channels];
        let unit = |channel: usize| pixel[channel].to_unit();
        match (color_type.has_color(), color_type.has_alpha()) {
            (false, false) => Color::rgb(unit(0), unit(0), unit(0)),
            (false, true) => Color::rgba(unit(0), unit(0), unit(0), unit(1)),
            (true, false) => Color::rgb(unit(0), unit(1), unit(2)),
            (true, true) => Color::rgba(unit(0), unit(1), unit(2), unit(3)),
        }
    }
    by_depth!(pixels, samples => read(samples, color_type, index))
}
