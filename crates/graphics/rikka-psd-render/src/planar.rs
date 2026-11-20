use rikka_simd::Level;

use crate::lanes::{Kernel, Lanes, Pixels, by_channels, sweep};

pub(crate) struct Planar {
    pub(crate) color: [Vec<f32>; 4],
    pub(crate) alpha: Vec<f32>,
}

impl Planar {
    pub(crate) fn new(width: usize) -> Self {
        Self {
            color: std::array::from_fn(|_| vec![0.0; width]),
            alpha: vec![0.0; width],
        }
    }

    pub(crate) fn join(&self, row: &mut [f32], channels: usize) {
        let count = self.alpha.len();
        let (color, alpha) = (&self.color, self.alpha.as_slice());
        by_channels!(channels, C => rikka_simd::dispatch!(Level::detect(), simd => {
            sweep(simd, &mut Join::<C> { row, color, alpha }, count)
        }));
    }
}

struct Join<'a, const C: usize> {
    row: &'a mut [f32],
    color: &'a [Vec<f32>; 4],
    alpha: &'a [f32],
}

impl<const C: usize> Kernel for Join<'_, C> {
    #[inline(always)]
    fn run<V: Lanes>(&mut self, at: usize) {
        let lanes = at..at + V::LANES;
        let mut color = [V::default(); C];
        for (value, plane) in color.iter_mut().zip(self.color) {
            *value = V::load(&plane[lanes.clone()]);
        }
        let alpha = V::load(&self.alpha[lanes]);
        Pixels { color, alpha }.store(self.row, at);
    }
}
