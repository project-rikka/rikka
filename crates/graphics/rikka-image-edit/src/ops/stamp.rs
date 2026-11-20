use std::collections::HashMap;
use std::sync::Arc;

use rikka_gpu::ImageFormat;

use crate::color::Space;
use crate::error::{Error, Result};
use crate::ops::Effect;
use crate::ops::passes::{Kernel1d, composite_over, resampled, scaled_extent};
use crate::picture::Frame;
use crate::work::Work;

#[derive(Clone, PartialEq)]
pub struct Stamp {
    width: u32,
    height: u32,
    pixels: Arc<[u8]>,
    left: i32,
    top: i32,
    scale: f32,
}

impl Stamp {
    pub fn new(width: u32, height: u32, pixels: impl Into<Arc<[u8]>>) -> Result<Stamp> {
        let pixels = pixels.into();
        let expected = width as usize * height as usize * 4;
        if width == 0 || height == 0 || pixels.len() != expected {
            return Err(Error::Invalid(format!(
                "{width}×{height} stamp needs {expected} bytes, got {}",
                pixels.len()
            )));
        }
        Ok(Stamp {
            width,
            height,
            pixels,
            left: 0,
            top: 0,
            scale: 1.0,
        })
    }

    #[must_use]
    pub fn with_position(mut self, left: i32, top: i32) -> Stamp {
        self.left = left;
        self.top = top;
        self
    }
}

impl Effect for Stamp {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let floats = translated(&self.pixels, &frame.space);
        let raw = work.plane([self.width, self.height], ImageFormat::Rgba32Float)?;
        crate::picture::upload(work, &raw, bytemuck::cast_slice(&floats));
        let placed = match self.scale == 1.0 {
            true => raw,
            false => resampled(
                work,
                &raw,
                scaled_extent([self.width, self.height], self.scale),
                Kernel1d::Box,
            )?,
        };
        let at = [self.left, self.top].map(|side| (side as f32 * self.scale).round() as i32);
        let out = composite_over(work, frame, &placed, at)?;
        Ok(frame.with_plane(out))
    }

    fn scaled(&self, factor: f32) -> Stamp {
        Stamp {
            scale: self.scale * factor,
            ..self.clone()
        }
    }
}

fn translated(pixels: &[u8], space: &Space) -> Vec<f32> {
    let mut converted: HashMap<[u8; 3], [f32; 3]> = HashMap::new();
    let mut last: Option<([u8; 3], [f32; 3])> = None;
    let mut floats = Vec::with_capacity(pixels.len());
    for pixel in pixels.as_chunks::<4>().0 {
        let [r, g, b, a] = *pixel;
        let alpha = f32::from(a) / 255.0;
        let key = [r, g, b];
        let color = match last {
            Some((cached_key, color)) if cached_key == key => color,
            _ => {
                let color = *converted.entry(key).or_insert_with(|| {
                    let [r, g, b, _] =
                        space.translated([r, g, b, 255].map(|channel| f32::from(channel) / 255.0));
                    [r, g, b]
                });
                last = Some((key, color));
                color
            }
        };
        floats.extend_from_slice(&[color[0], color[1], color[2], alpha]);
    }
    floats
}

impl std::fmt::Debug for Stamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stamp")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("left", &self.left)
            .field("top", &self.top)
            .field("scale", &self.scale)
            .finish_non_exhaustive()
    }
}
