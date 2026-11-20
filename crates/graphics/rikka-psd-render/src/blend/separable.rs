use crate::lanes::Lanes;

pub(crate) trait Blend {
    const CLAMPS: bool;

    fn mix<V: Lanes>(cb: V, cs: V) -> V;
}

macro_rules! separable {
    ($($mode:ident<$v:ident>($cb:ident, $cs:ident) => $formula:expr;)*) => {$(
        pub(super) struct $mode;

        impl Blend for $mode {
            const CLAMPS: bool = true;

            #[inline(always)]
            fn mix<$v: Lanes>($cb: $v, $cs: $v) -> $v {
                $formula
            }
        }
    )*};
}

pub(super) struct Normal;

impl Blend for Normal {
    const CLAMPS: bool = false;

    #[inline(always)]
    fn mix<V: Lanes>(_: V, cs: V) -> V {
        cs
    }
}

#[inline(always)]
fn hard_light<V: Lanes>(cb: V, cs: V) -> V {
    let s = V::splat(2.0) * cs - V::splat(1.0);
    V::select(
        cs.simd_le(V::splat(0.5)),
        cb * V::splat(2.0) * cs,
        cb + s - cb * s,
    )
}

separable! {
    Multiply<V>(cb, cs) => cb * cs;
    Screen<V>(cb, cs) => cb + cs - cb * cs;
    Overlay<V>(cb, cs) => hard_light(cs, cb);
    Darken<V>(cb, cs) => cb.min(cs);
    Lighten<V>(cb, cs) => cb.max(cs);
    ColorDodge<V>(cb, cs) => {
        let one = V::splat(1.0);
        let dodged = V::select(cs.simd_ge(one), one, (cb / (one - cs)).min(one));
        V::select(cb.simd_le(V::default()), V::default(), dodged)
    };
    ColorBurn<V>(cb, cs) => {
        let one = V::splat(1.0);
        let burned = V::select(
            cs.simd_le(V::default()),
            V::default(),
            one - ((one - cb) / cs).min(one),
        );
        V::select(cb.simd_ge(one), one, burned)
    };
    LinearDodge<V>(cb, cs) => (cb + cs).min(V::splat(1.0));
    LinearBurn<V>(cb, cs) => (cb + cs - V::splat(1.0)).max(V::default());
    HardLight<V>(cb, cs) => hard_light(cb, cs);
    SoftLight<V>(cb, cs) => {
        let one = V::splat(1.0);
        let two = V::splat(2.0);
        let darkened = cb - (one - two * cs) * cb * (one - cb);
        let polynomial = ((V::splat(16.0) * cb - V::splat(12.0)) * cb + V::splat(4.0)) * cb;
        let d = V::select(cb.simd_le(V::splat(0.25)), polynomial, cb.sqrt());
        let lightened = cb + (two * cs - one) * (d - cb);
        V::select(cs.simd_le(V::splat(0.5)), darkened, lightened)
    };
    VividLight<V>(cb, cs) => {
        let one = V::splat(1.0);
        let two = V::splat(2.0);
        let burned = one - ((one - cb) / (two * cs)).min(one);
        let dodged = (cb / (two * (one - cs))).min(one);
        let lit = V::select(cs.simd_le(V::splat(0.5)), burned, dodged);
        let lit = V::select(cs.simd_ge(one), one, lit);
        V::select(cs.simd_le(V::default()), V::default(), lit)
    };
    LinearLight<V>(cb, cs) => {
        (cb + V::splat(2.0) * cs - V::splat(1.0)).clamp(V::default(), V::splat(1.0))
    };
    PinLight<V>(cb, cs) => {
        let doubled = V::splat(2.0) * cs;
        V::select(
            cs.simd_gt(V::splat(0.5)),
            cb.max(doubled - V::splat(1.0)),
            cb.min(doubled),
        )
    };
    HardMix<V>(cb, cs) => {
        let one = V::splat(1.0);
        let total = cb + cs;
        let full = total.simd_gt(one) | (total.simd_eq(one) & cb.simd_gt(V::splat(0.5)));
        V::select(full, one, V::default())
    };
    Difference<V>(cb, cs) => (cb - cs).abs();
    Exclusion<V>(cb, cs) => cb + cs - V::splat(2.0) * cb * cs;
    Subtract<V>(cb, cs) => (cb - cs).max(V::default());
    Divide<V>(cb, cs) => {
        let one = V::splat(1.0);
        let empty = V::select(cb.simd_le(V::default()), V::default(), one);
        V::select(cs.simd_le(V::default()), empty, (cb / cs).min(one))
    };
}
