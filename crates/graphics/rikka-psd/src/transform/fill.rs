use rikka_types::srgb;

use crate::color::Color;
use crate::document::{ColorMode, Document};
use crate::pixels::Depth;
use crate::transform::records::has_background;

impl Document {
    pub fn native_color(&self, color: Color) -> Vec<f32> {
        let linear = |value: f64| {
            if self.depth == Depth::ThirtyTwo {
                srgb::to_linear_f64(value.clamp(0.0, 1.0))
            } else {
                value
            }
        };
        match (self.color_mode, color) {
            (ColorMode::Cmyk, Color::Cmyk { c, m, y, k }) => {
                [c, m, y, k].map(|ink| (1.0 - ink) as f32).to_vec()
            }
            (ColorMode::Grayscale | ColorMode::Duotone, Color::Gray { k }) => {
                vec![linear(1.0 - k) as f32]
            }
            (ColorMode::Indexed, color) => {
                let rgb = color.to_rgb().map(|value| value.clamp(0.0, 1.0) * 255.0);
                let index = self
                    .color_mode_data
                    .palette()
                    .unwrap_or(&[])
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| distance(rgb, **a).total_cmp(&distance(rgb, **b)))
                    .map_or(0, |(index, _)| index);
                vec![index as f32 / 255.0]
            }
            (_, color) => {
                let rgb = color.to_rgb().map(|value| linear(value) as f32);
                self.color_converter()
                    .from_rgb(&[rgb])
                    .into_iter()
                    .map(|channel| channel[0])
                    .collect()
            }
        }
    }

    pub(crate) fn canvas_fill(&self, background: Color) -> Vec<f32> {
        let opaque = has_background(self) || (self.layers.is_empty() && !self.merged_alpha);
        let color = self.native_color(if opaque { background } else { Color::WHITE });
        let alpha = self.composite_alpha_index();
        (0..self.composite.planes.len())
            .map(|index| match color.get(index) {
                Some(&value) => value,
                None if Some(index) == alpha => f32::from(u8::from(opaque)),
                None if self.color_mode == ColorMode::Multichannel => 1.0,
                None => 0.0,
            })
            .collect()
    }
}

fn distance(rgb: [f64; 3], entry: [u8; 3]) -> f64 {
    rgb.iter()
        .zip(entry)
        .map(|(a, b)| (a - f64::from(b)).powi(2))
        .sum()
}
