use rikka_simd::{Float, Simd, Vector};

const BLOCK: usize = 64;
const CHAINS: usize = 4;

#[inline(always)]
pub(crate) fn convolve<S: Simd, const C: usize>(
    _: S,
    weights: &[f32],
    windows: [&[f32]; C],
) -> [f32; C] {
    let lanes = S::F32s::LANES;
    let (blocks, rest) = weights.as_chunks::<BLOCK>();
    let windows = windows.map(|window| window[..weights.len()].as_chunks::<BLOCK>());
    let mut chains = [[S::F32s::default(); CHAINS]; C];
    for (index, block) in blocks.iter().enumerate() {
        for (part, at) in (0..BLOCK).step_by(lanes).enumerate() {
            let weight = S::F32s::load(&block[at..]);
            for (chain, (window, _)) in chains.iter_mut().zip(&windows) {
                let link = &mut chain[part % CHAINS];
                *link = S::F32s::load(&window[index][at..]).mul_add(weight, *link);
            }
        }
    }
    for (part, at) in (0..rest.len()).step_by(lanes).enumerate() {
        let weight = S::F32s::load_partial(&rest[at..]);
        for (chain, (_, window)) in chains.iter_mut().zip(&windows) {
            let link = &mut chain[part % CHAINS];
            *link = S::F32s::load_partial(&window[at..]).mul_add(weight, *link);
        }
    }
    let mut sums = [0.0; C];
    for (sum, [a, b, c, d]) in sums.iter_mut().zip(chains) {
        *sum = ((a + b) + (c + d)).reduce_add();
    }
    sums
}
