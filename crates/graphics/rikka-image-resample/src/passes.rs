use rikka_simd::{Simd, Vector};

use crate::taps::Taps;

#[inline(always)]
pub(crate) fn across<S: Simd, const C: usize>(
    _: S,
    source: &[f32],
    taps: &Taps,
    target: &mut [f32],
) {
    if C == 4 {
        for (pixel, &tap) in target.as_chunks_mut::<4>().0.iter_mut().zip(&taps.taps) {
            let texels = &source[tap.first * 4..(tap.first + tap.count) * 4];
            let mut sum = S::F32x4::default();
            for (texel, &weight) in texels.as_chunks::<4>().0.iter().zip(taps.weights_of(tap)) {
                sum += S::F32x4::load(texel) * S::F32x4::splat(weight);
            }
            sum.store(pixel);
        }
        return;
    }
    for (pixel, &tap) in target.as_chunks_mut::<C>().0.iter_mut().zip(&taps.taps) {
        let texels = &source[tap.first * C..(tap.first + tap.count) * C];
        let mut sum = [0.0f32; C];
        for (texel, &weight) in texels.as_chunks::<C>().0.iter().zip(taps.weights_of(tap)) {
            for channel in 0..C {
                sum[channel] += texel[channel] * weight;
            }
        }
        *pixel = sum;
    }
}

#[inline(always)]
pub(crate) fn down<S: Simd>(_: S, row: &[f32], weight: f32, target: &mut [f32]) {
    let lanes = S::F32s::LANES;
    let factor = S::F32s::splat(weight);
    let (blocks, rest) = target.split_at_mut(target.len() / lanes * lanes);
    for (sums, values) in blocks.chunks_exact_mut(lanes).zip(row.chunks_exact(lanes)) {
        (S::F32s::load(sums) + S::F32s::load(values) * factor).store(sums);
    }
    let values = &row[blocks.len()..];
    for (sum, &value) in rest.iter_mut().zip(values) {
        *sum += value * weight;
    }
}
