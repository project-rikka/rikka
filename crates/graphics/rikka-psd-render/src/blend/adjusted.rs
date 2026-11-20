use rikka_psd::BlendMode;
use rikka_simd::{Simd, Sse2, Vector};

use crate::blend::{Blend, Blending, blend, blending};
use crate::space::Space;

type One = <Sse2 as Simd>::F32x1;

pub(crate) fn blend_adjusted(
    adjusted: &mut [f32],
    backdrop: &[f32],
    mode: BlendMode,
    space: &Space,
) {
    blending(
        mode,
        Adjusted {
            adjusted,
            backdrop,
            mode,
            space,
        },
    );
}

struct Adjusted<'a> {
    adjusted: &'a mut [f32],
    backdrop: &'a [f32],
    mode: BlendMode,
    space: &'a Space,
}

impl Adjusted<'_> {
    fn each(self, mix: impl Fn(&[f32; 4], &[f32; 4], &mut [f32; 4])) {
        let channels = self.space.channels();
        let mut cb = [0.0f32; 4];
        let mut cs = [0.0f32; 4];
        let mut out = [0.0f32; 4];
        for (pixel, base) in self
            .adjusted
            .chunks_exact_mut(channels + 1)
            .zip(self.backdrop.chunks_exact(channels + 1))
        {
            let alpha = base[channels];
            if alpha <= 0.0 {
                continue;
            }
            for channel in 0..channels {
                cb[channel] = base[channel] / alpha;
                cs[channel] = pixel[channel] / alpha;
            }
            mix(&cb, &cs, &mut out);
            for channel in 0..channels {
                pixel[channel] = out[channel] * alpha;
            }
        }
    }
}

impl Blending for Adjusted<'_> {
    type Output = ();

    fn separable<B: Blend>(self) {
        let channels = self.space.channels();
        self.each(|cb, cs, out| {
            for ((slot, &cb), &cs) in out[..channels].iter_mut().zip(cb).zip(cs) {
                let (cb, cs) = match B::CLAMPS {
                    true => (cb.clamp(0.0, 1.0), cs.clamp(0.0, 1.0)),
                    false => (cb, cs),
                };
                *slot = B::mix(One::splat(cb), One::splat(cs)).lane(0);
            }
        });
    }

    fn per_pixel(self) {
        let (mode, space) = (self.mode, self.space);
        self.each(|cb, cs, out| blend(mode, space, cb, cs, out));
    }
}
