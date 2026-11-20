use rikka_simd::{Float, Narrow, NarrowUnsigned, Simd, Vector};

use crate::fill::rule::FillRule;

pub(crate) const BLOCK: usize = 8;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Grid {
    pub(crate) stride: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
}

#[inline(always)]
pub(crate) fn resolve_mask<S: Simd>(cells: &mut [f32], grid: Grid, out: &mut [u8]) {
    debug_assert!(cells.len() >= grid.stride * grid.height);
    debug_assert!(out.len() >= grid.width * grid.height);
    let ceiling = S::F32x8::splat(1.0);
    let scale = S::F32x8::splat(255.0);
    for row in 0..grid.height {
        let cells = &mut cells[row * grid.stride..(row + 1) * grid.stride];
        let line = row * grid.width;
        let mut carry = S::F32x8::default();
        for (index, block) in cells.as_chunks_mut::<BLOCK>().0.iter_mut().enumerate() {
            let x = index * BLOCK;
            let prefix = S::F32x8::load(block).prefix_sum();
            let sums = prefix + carry;
            carry += S::F32x8::splat(prefix.lane(BLOCK - 1));
            block.fill(0.0);
            if x < grid.width {
                let levels = (sums.abs().min(ceiling) * scale).to_int_round();
                let words = Narrow::narrow(levels, S::I32x8::default());
                let bytes = NarrowUnsigned::narrow_unsigned(words, S::I16x16::default());
                let mut lanes = [0u8; 32];
                bytes.store(&mut lanes);
                store(
                    out,
                    line + x,
                    lanes
                        .first_chunk::<BLOCK>()
                        .expect("lane buffer shorter than BLOCK"),
                );
            }
        }
    }
}

pub(crate) fn resolve_coverage(cells: &mut [f32], grid: Grid, rule: FillRule, out: &mut [f32]) {
    for row in 0..grid.height {
        let cells = &mut cells[row * grid.stride..(row + 1) * grid.stride];
        let out = &mut out[row * grid.width..(row + 1) * grid.width];
        let mut sum = 0.0f32;
        match rule {
            FillRule::NonZero => {
                for (target, cell) in out.iter_mut().zip(cells.iter()) {
                    sum += *cell;
                    *target = sum.abs().min(1.0);
                }
            }
            FillRule::EvenOdd => {
                for (target, cell) in out.iter_mut().zip(cells.iter()) {
                    sum += *cell;
                    let folded = sum.abs() % 2.0;
                    *target = if folded > 1.0 { 2.0 - folded } else { folded };
                }
            }
        }
        cells.fill(0.0);
    }
}

#[inline(always)]
pub(crate) fn store<const N: usize>(out: &mut [u8], at: usize, bytes: &[u8; N]) {
    match out.get_mut(at..at + N) {
        Some(target) => target.copy_from_slice(bytes),
        None => {
            for (target, &byte) in out[at..].iter_mut().zip(bytes) {
                *target = byte;
            }
        }
    }
}
