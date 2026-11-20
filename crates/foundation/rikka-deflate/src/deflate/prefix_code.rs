use crate::tables::MAX_CODE_BITS;

const MAX_SYMBOLS: usize = 288;
const MAX_NODES: usize = MAX_SYMBOLS * 2;

#[derive(Clone, Copy, Debug)]
pub(crate) struct PrefixCode<const N: usize> {
    pub(crate) lengths: [u8; N],
    pub(crate) codes: [u16; N],
}

impl<const N: usize> PrefixCode<N> {
    pub(crate) fn from_counts(counts: &[u32; N], limit: usize) -> Self {
        let mut lengths = [0u8; N];
        limited_lengths(counts, limit, &mut lengths);
        Self::from_lengths(lengths)
    }

    pub(crate) fn from_lengths(lengths: [u8; N]) -> Self {
        let mut used = [0u16; MAX_CODE_BITS + 1];
        for &length in &lengths {
            used[usize::from(length)] += 1;
        }
        used[0] = 0;
        let mut next = [0u16; MAX_CODE_BITS + 1];
        let mut code = 0u16;
        for bits in 1..=MAX_CODE_BITS {
            code = (code + used[bits - 1]) << 1;
            next[bits] = code;
        }
        let mut codes = [0u16; N];
        for (code, &length) in codes.iter_mut().zip(&lengths) {
            if length != 0 {
                let bits = usize::from(length);
                *code = next[bits].reverse_bits() >> (16 - bits);
                next[bits] += 1;
            }
        }
        Self { lengths, codes }
    }

    pub(crate) fn cost(&self, counts: &[u32; N]) -> u64 {
        counts
            .iter()
            .zip(&self.lengths)
            .map(|(&count, &length)| u64::from(count) * u64::from(length))
            .sum()
    }
}

fn limited_lengths(counts: &[u32], limit: usize, lengths: &mut [u8]) {
    let mut leaves = [(0u32, 0u16); MAX_SYMBOLS];
    let mut used = 0;
    for (symbol, &count) in counts.iter().enumerate() {
        if count != 0 {
            leaves[used] = (count, symbol as u16);
            used += 1;
        }
    }
    for filler in 0..2u16 {
        if used < 2 && !leaves[..used].iter().any(|&(_, symbol)| symbol == filler) {
            leaves[used] = (1, filler);
            used += 1;
        }
    }
    let leaves = &mut leaves[..used];
    leaves.sort_unstable();

    let mut weights = [0u64; MAX_NODES];
    let mut parents = [0u16; MAX_NODES];
    for (weight, &(count, _)) in weights.iter_mut().zip(leaves.iter()) {
        *weight = u64::from(count);
    }
    let root = 2 * used - 2;
    let mut leaf = 0;
    let mut branch = used;
    for node in used..=root {
        let mut weight = 0;
        for _ in 0..2 {
            let takes_leaf = leaf < used && (branch >= node || weights[leaf] <= weights[branch]);
            let child = if takes_leaf {
                leaf += 1;
                leaf - 1
            } else {
                branch += 1;
                branch - 1
            };
            weight += weights[child];
            parents[child] = node as u16;
        }
        weights[node] = weight;
    }

    let mut depths = [0u16; MAX_NODES];
    for node in (0..root).rev() {
        depths[node] = depths[usize::from(parents[node])] + 1;
    }
    let mut histogram = [0u32; MAX_CODE_BITS + 1];
    for &depth in &depths[..used] {
        histogram[usize::from(depth).min(limit)] += 1;
    }
    let mut total: u32 = (1..=limit)
        .map(|bits| histogram[bits] << (limit - bits))
        .sum();
    while total > 1 << limit {
        histogram[limit] -= 1;
        for bits in (1..limit).rev() {
            if histogram[bits] > 0 {
                histogram[bits] -= 1;
                histogram[bits + 1] += 2;
                break;
            }
        }
        total -= 1;
    }
    let mut ordered = leaves.iter();
    for bits in (1..=limit).rev() {
        for &(_, symbol) in ordered.by_ref().take(histogram[bits] as usize) {
            lengths[usize::from(symbol)] = bits as u8;
        }
    }
}
