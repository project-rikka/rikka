use rikka_simd::prelude::*;

use super::field::{NIBBLES, mul};

const NIBBLE: u8 = 0x0F;

pub(super) fn combine(level: Level, coefficients: &[u8], sources: &[&[u8]], out: &mut [u8]) {
    rikka_simd::dispatch!(level, simd => accumulate(simd, coefficients, sources, out));
}

#[inline(always)]
fn accumulate<S: Simd>(simd: S, coefficients: &[u8], sources: &[&[u8]], out: &mut [u8]) {
    for (&coefficient, source) in coefficients.iter().zip(sources) {
        match coefficient {
            0 => {}
            1 => xor_into(simd, source, out),
            _ => mul_into(simd, coefficient, source, out),
        }
    }
}

#[inline(always)]
fn xor_into<S: Simd>(_: S, source: &[u8], out: &mut [u8]) {
    let lanes = S::U8s::LANES;
    let whole = source.len().min(out.len()) / lanes * lanes;
    for (from, to) in source[..whole]
        .chunks_exact(lanes)
        .zip(out[..whole].chunks_exact_mut(lanes))
    {
        (S::U8s::load(to) ^ S::U8s::load(from)).store(to);
    }
    for (from, to) in source[whole..].iter().zip(&mut out[whole..]) {
        *to ^= from;
    }
}

#[inline(always)]
fn mul_into<S: Simd>(_: S, coefficient: u8, source: &[u8], out: &mut [u8]) {
    let length = source.len().min(out.len());
    let mut done = 0;
    if S::SSE42 {
        let low = S::U8x16::load(&NIBBLES.low[usize::from(coefficient)]);
        let high = S::U8x16::load(&NIBBLES.high[usize::from(coefficient)]);
        let mask = S::U8x16::splat(NIBBLE);
        let lanes = S::U8x16::LANES;
        done = length / lanes * lanes;
        for (from, to) in source[..done]
            .chunks_exact(lanes)
            .zip(out[..done].chunks_exact_mut(lanes))
        {
            let value = S::U8x16::load(from);
            let product = low.swizzle_dyn(value & mask) ^ high.swizzle_dyn((value >> 4) & mask);
            (S::U8x16::load(to) ^ product).store(to);
        }
    }
    for (from, to) in source[done..length].iter().zip(&mut out[done..length]) {
        *to ^= mul(coefficient, *from);
    }
}
