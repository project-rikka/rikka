use rikka_simd::ScalarFloat;

#[inline(always)]
pub(crate) fn ceil_to_i32(value: f32) -> i32 {
    let truncated = value.to_int_trunc();
    truncated + i32::from((truncated as f32) < value)
}

#[inline(always)]
pub(crate) fn floor_to_i32(value: f32) -> i32 {
    let truncated = value.to_int_trunc();
    truncated - i32::from(value < (truncated as f32))
}
