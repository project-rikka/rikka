use rikka_simd::{Float, Simd, Vector};

pub(crate) const STENCIL: usize = 4;

pub(crate) struct Phases<'a> {
    pub(crate) rows: [&'a [f32]; STENCIL],
    pub(crate) coefficients: [f32; STENCIL],
}

impl Phases<'_> {
    #[inline(always)]
    pub(crate) fn blend<S: Simd>(&self, _: S, weights: &mut [f32]) {
        let [first, second, third, fourth] = self.rows;
        let [a, b, c, d] = self.coefficients;
        let [a, b, c, d] = [
            S::F32s::splat(a),
            S::F32s::splat(b),
            S::F32s::splat(c),
            S::F32s::splat(d),
        ];
        for at in (0..weights.len()).step_by(S::F32s::LANES) {
            let weight = S::F32s::load_partial(&first[at..]) * a;
            let weight = S::F32s::load_partial(&second[at..]).mul_add(b, weight);
            let weight = S::F32s::load_partial(&third[at..]).mul_add(c, weight);
            let weight = S::F32s::load_partial(&fourth[at..]).mul_add(d, weight);
            weight.store_partial(&mut weights[at..]);
        }
    }
}
