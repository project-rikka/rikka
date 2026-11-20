use rikka_simd::{Int, Vector};

use crate::lanes::Lanes;

pub(crate) fn sample(table: &[f32], value: f32) -> f32 {
    let last = table.len() - 1;
    let position = value.clamp(0.0, 1.0) * last as f32;
    let low = position as usize;
    let high = (low + 1).min(last);
    table[low] + (table[high] - table[low]) * (position - low as f32)
}

#[inline(always)]
pub(crate) fn sample_lanes<V: Lanes>(table: &[f32], value: V) -> V {
    let last = table.len() - 1;
    let position = value.clamp(V::default(), V::splat(1.0)) * V::splat(last as f32);
    let low = position.to_int_trunc();
    let high = (low + V::Int::splat(1)).min(V::Int::splat(last as i32));
    let below = V::gather(table, low.reinterpret());
    let above = V::gather(table, high.reinterpret());
    below + (above - below) * (position - V::from_int(low))
}
