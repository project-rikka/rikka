use rikka_simd::{Float, Level, Narrow, NarrowUnsigned, Simd, Vector};

const MAX_LANES: usize = 64;

pub(crate) fn quantize_u8(values: &[f32], out: &mut [u8]) {
    rikka_simd::dispatch!(
        Level::detect(),
        simd => pack_all(values, out, |from| pack_bytes(simd, from))
    );
}

pub(crate) fn quantize_u16(values: &[f32], out: &mut [u16]) {
    rikka_simd::dispatch!(
        Level::detect(),
        simd => pack_all(values, out, |from| pack_words(simd, from, 65535.0))
    );
}

#[inline(always)]
fn pack_all<V: Vector>(values: &[f32], out: &mut [V::Elem], pack: impl Fn(&[f32]) -> V) {
    let step = V::LANES;
    let whole = values.len() / step * step;
    for (from, to) in values[..whole]
        .chunks_exact(step)
        .zip(out[..whole].chunks_exact_mut(step))
    {
        pack(from).store(to);
    }
    let rest = values.len() - whole;
    if rest > 0 {
        let mut from = [0.0f32; MAX_LANES];
        let mut to = [V::Elem::default(); MAX_LANES];
        from[..rest].copy_from_slice(&values[whole..]);
        pack(&from[..step]).store(&mut to[..step]);
        out[whole..].copy_from_slice(&to[..rest]);
    }
}

#[inline(always)]
fn pack_bytes<S: Simd>(simd: S, values: &[f32]) -> S::U8s {
    let half = S::U16s::LANES;
    S::U16s::narrow(
        pack_words(simd, &values[..half], 255.0),
        pack_words(simd, &values[half..], 255.0),
    )
}

#[inline(always)]
fn pack_words<S: Simd>(_: S, values: &[f32], scale: f32) -> S::U16s {
    let lanes = S::F32s::LANES;
    S::I32s::narrow_unsigned(
        level::<S>(S::F32s::load(&values[..lanes]), scale),
        level::<S>(S::F32s::load(&values[lanes..]), scale),
    )
}

#[inline(always)]
fn level<S: Simd>(value: S::F32s, scale: f32) -> S::I32s {
    let scaled = value.clamp(S::F32s::default(), S::F32s::splat(1.0)) * S::F32s::splat(scale);
    let whole = scaled.to_int_trunc();
    let up = (scaled - S::F32s::from_int(whole)).simd_ge(S::F32s::splat(0.5));
    whole + S::I32s::select(up, S::I32s::splat(1), S::I32s::default())
}
