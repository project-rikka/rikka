use std::borrow::Borrow;
use std::ops::Range;

use rikka_psd::Rect;

use crate::mask::{Mask, coverage_row};
#[derive(Debug, Clone, PartialEq)]
pub struct Raster {
    pub(crate) bounds: Rect,
    pub(crate) channels: usize,
    pub(crate) data: Vec<f32>,
}

impl Raster {
    pub(crate) fn new(bounds: Rect, channels: usize) -> Self {
        Self {
            bounds,
            channels,
            data: vec![0.0; bounds.area() as usize * (channels + 1)],
        }
    }

    pub(crate) fn stride(&self) -> usize {
        self.channels + 1
    }

    pub fn width(&self) -> u32 {
        self.bounds.width()
    }

    pub fn height(&self) -> u32 {
        self.bounds.height()
    }

    pub(crate) fn index(&self, x: i32, y: i32) -> Option<usize> {
        if !self.bounds.contains(x, y) {
            return None;
        }
        let column = (x - self.bounds.left) as usize;
        let row = (y - self.bounds.top) as usize;
        Some((row * self.bounds.width() as usize + column) * self.stride())
    }

    pub(crate) fn span(&self, y: i32, columns: Range<i32>) -> Range<usize> {
        let start = self
            .index(columns.start, y)
            .expect("span starts inside raster");
        start..start + columns.len() * self.stride()
    }

    pub fn pixel(&self, x: i32, y: i32) -> Option<&[f32]> {
        let index = self.index(x, y)?;
        Some(&self.data[index..index + self.stride()])
    }

    pub(crate) fn straight(&self, x: i32, y: i32, out: &mut [f32]) -> f32 {
        let Some(index) = self.index(x, y) else {
            out.iter_mut().for_each(|value| *value = 0.0);
            return 0.0;
        };
        let alpha = self.data[index + self.channels];
        for (slot, value) in out.iter_mut().zip(&self.data[index..index + self.channels]) {
            *slot = if alpha > 0.0 { value / alpha } else { 0.0 };
        }
        alpha
    }

    pub(crate) fn region(&self, bounds: Rect) -> Raster {
        let mut out = Raster::new(bounds, self.channels);
        out.copy_from(self);
        out
    }

    pub(crate) fn copy_from(&mut self, source: &Raster) {
        let overlap = self.bounds.intersection(&source.bounds);
        if overlap.is_empty() {
            return;
        }
        let stride = self.stride();
        let span = overlap.width() as usize * stride;
        for y in overlap.top..overlap.bottom {
            let from = source.index(overlap.left, y).expect("inside source");
            let to = self.index(overlap.left, y).expect("inside target");
            self.data[to..to + span].copy_from_slice(&source.data[from..from + span]);
        }
    }

    pub(crate) fn alpha_mask(&self) -> Mask {
        let mut mask = Mask::new(self.bounds, 0.0);
        for (slot, pixel) in mask
            .data
            .iter_mut()
            .zip(self.data.chunks_exact(self.stride()))
        {
            *slot = pixel[self.channels];
        }
        mask
    }

    pub(crate) fn multiply_alpha(&mut self, mask: &Mask) {
        let bounds = self.bounds;
        if bounds.is_empty() {
            return;
        }
        let stride = self.stride();
        let mut factors = vec![0.0; bounds.width() as usize];
        for y in bounds.top..bounds.bottom {
            mask.row_into(y, bounds.left, &mut factors);
            let span = self.span(y, bounds.left..bounds.right);
            for (pixel, &factor) in self.data[span].chunks_exact_mut(stride).zip(&factors) {
                if factor < 1.0 {
                    pixel.iter_mut().for_each(|value| *value *= factor);
                }
            }
        }
    }

    pub(crate) fn scale(&mut self, factor: f32) {
        if factor != 1.0 {
            self.data.iter_mut().for_each(|value| *value *= factor);
        }
    }

    pub(crate) fn lerp_from(&mut self, target: &Raster, amount: f32, masks: &[impl Borrow<Mask>]) {
        let overlap = self.bounds.intersection(&target.bounds);
        let mut factors = vec![0.0; overlap.width() as usize];
        for y in overlap.top..overlap.bottom {
            coverage_row(amount, masks, y, overlap.left, &mut factors);
            let to = self.span(y, overlap.left..overlap.right);
            let from = target.span(y, overlap.left..overlap.right);
            lerp_row(
                &mut self.data[to],
                &target.data[from],
                &factors,
                self.channels + 1,
            );
        }
    }
}

pub(crate) fn lerp_row(row: &mut [f32], target: &[f32], factors: &[f32], stride: usize) {
    let pixels = row
        .chunks_exact_mut(stride)
        .zip(target.chunks_exact(stride));
    for ((pixel, toward), &factor) in pixels.zip(factors) {
        if factor > 0.0 {
            for (value, toward) in pixel.iter_mut().zip(toward) {
                *value += (toward - *value) * factor;
            }
        }
    }
}
