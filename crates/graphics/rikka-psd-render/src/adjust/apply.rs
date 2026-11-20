use rikka_simd::{Level, Mask as _};

use crate::adjust::{ChannelLuts, Operation};
use crate::lanes::{Kernel, Lanes, Pixels, by_channels, sweep};
use crate::space::{Space, SpaceKind};
use crate::table::sample_lanes;

pub(crate) struct Adjuster<'a> {
    operation: &'a Operation,
    space: Space,
}

impl Operation {
    pub(crate) fn adjuster(&self, space: Space) -> Adjuster<'_> {
        Adjuster {
            operation: self,
            space,
        }
    }
}

impl Adjuster<'_> {
    pub(crate) fn apply(&self, row: &mut [f32]) {
        let space = &self.space;
        match (self.operation, space.kind) {
            (Operation::Invert, _) => {
                recolor(space, row, |straight| straight.map(|value| 1.0 - value))
            }
            (Operation::Luts(luts), SpaceKind::Lab) => recolor(space, row, |straight| {
                rgb(space, straight, |rgb| luts.apply(rgb))
            }),
            (Operation::Luts(luts), _) => apply_luts(space, row, luts),
            (Operation::Pixel(function), _) => {
                recolor(space, row, |straight| rgb(space, straight, function))
            }
        }
    }
}

fn apply_luts(space: &Space, row: &mut [f32], luts: &ChannelLuts) {
    let channels = space.channels();
    let count = row.len() / (channels + 1);
    by_channels!(channels, C => rikka_simd::dispatch!(Level::detect(), simd => {
        sweep(simd, &mut LutKernel::<C> { luts, row }, count)
    }));
}

struct LutKernel<'a, const C: usize> {
    luts: &'a ChannelLuts,
    row: &'a mut [f32],
}

impl<const C: usize> Kernel for LutKernel<'_, C> {
    #[inline(always)]
    fn run<V: Lanes>(&mut self, at: usize) {
        let mut pixels = Pixels::<V, C>::load(self.row, at);
        let alpha = pixels.alpha;
        let visible = alpha.simd_gt(V::default());
        if !visible.any() {
            return;
        }
        for (value, table) in pixels.color.iter_mut().zip(&self.luts.channels) {
            let mut toned = *value / alpha;
            if let Some(table) = table {
                toned = sample_lanes(table, toned);
            }
            if let Some(table) = &self.luts.master {
                toned = sample_lanes(table, toned);
            }
            let stored = toned.clamp(V::default(), V::splat(1.0)) * alpha;
            *value = V::select(visible, stored, *value);
        }
        pixels.store(self.row, at);
    }
}

fn rgb(space: &Space, straight: [f32; 4], function: impl Fn([f32; 3]) -> [f32; 3]) -> [f32; 4] {
    let mut out = [0.0f32; 4];
    space.encode_rgb(function(space.to_rgb(&straight)), &mut out);
    out
}

fn recolor(space: &Space, row: &mut [f32], function: impl Fn([f32; 4]) -> [f32; 4]) {
    let channels = space.channels();
    let mut straight = [0.0f32; 4];
    for pixel in row.chunks_exact_mut(channels + 1) {
        let alpha = pixel[channels];
        if alpha <= 0.0 {
            continue;
        }
        for channel in 0..channels {
            straight[channel] = pixel[channel] / alpha;
        }
        let adjusted = function(straight);
        for channel in 0..channels {
            pixel[channel] = adjusted[channel].clamp(0.0, 1.0) * alpha;
        }
    }
}
