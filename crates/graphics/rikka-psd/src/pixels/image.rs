use rikka_types::srgb;

use crate::error::{Error, Result};
use crate::pixels::plane::to_u8;
use crate::pixels::quantize::quantize_u8;

#[derive(Debug, Clone, PartialEq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[f32; 4]>,
    pub linear: bool,
}

impl RgbaImage {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![[0.0; 4]; width as usize * height as usize],
            linear: false,
        }
    }

    pub fn from_rgba8(width: u32, height: u32, bytes: &[u8]) -> Result<Self> {
        if bytes.len() != width as usize * height as usize * 4 {
            return Err(Error::Planes("RGBA bytes do not match the size"));
        }
        Ok(Self {
            width,
            height,
            pixels: bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|pixel| pixel.map(|value| f32::from(value) / 255.0))
                .collect(),
            linear: false,
        })
    }

    pub fn get(&self, x: u32, y: u32) -> [f32; 4] {
        self.pixels[y as usize * self.width as usize + x as usize]
    }

    pub fn to_rgba8(&self) -> Vec<u8> {
        let mut bytes = vec![0u8; self.pixels.len() * 4];
        encode_rgba8(&self.pixels, self.linear, &mut bytes);
        bytes
    }

    pub fn to_linear(&self) -> RgbaImage {
        if self.linear {
            return self.clone();
        }
        RgbaImage {
            width: self.width,
            height: self.height,
            pixels: self
                .pixels
                .iter()
                .map(|pixel| {
                    let channel = |value: f32| srgb::to_linear_f64(f64::from(value)) as f32;
                    [
                        channel(pixel[0]),
                        channel(pixel[1]),
                        channel(pixel[2]),
                        pixel[3],
                    ]
                })
                .collect(),
            linear: true,
        }
    }
}

pub(crate) fn encode_rgba8(pixels: &[[f32; 4]], linear: bool, out: &mut [u8]) {
    if !linear {
        quantize_u8(pixels.as_flattened(), out);
        return;
    }
    let encode = |value: f32| to_u8(srgb::from_linear_f64(f64::from(value.clamp(0.0, 1.0))) as f32);
    for ([r, g, b, a], bytes) in pixels.iter().zip(out.as_chunks_mut::<4>().0) {
        *bytes = [encode(*r), encode(*g), encode(*b), to_u8(*a)];
    }
}
