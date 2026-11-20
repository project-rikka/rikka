#[inline(always)]
pub(crate) fn larger(a: f32, b: f32) -> f32 {
    if b > a { b } else { a }
}

#[inline(always)]
pub(crate) fn smaller(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}
