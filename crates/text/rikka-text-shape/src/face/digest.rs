use rikka_text_font::Coverage;

const SHIFTS: [u32; 3] = [4, 0, 6];
const MASK_BITS: u64 = 63;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Digest {
    masks: [u64; 3],
}

impl Digest {
    #[inline]
    pub(crate) fn add(&mut self, glyph: u32) {
        for (mask, shift) in self.masks.iter_mut().zip(SHIFTS) {
            *mask |= 1 << ((u64::from(glyph) >> shift) & MASK_BITS);
        }
    }

    pub(crate) fn add_range(&mut self, first: u32, last: u32) {
        let (first, last) = (u64::from(first), u64::from(last));
        for (mask, shift) in self.masks.iter_mut().zip(SHIFTS) {
            if (last >> shift).wrapping_sub(first >> shift) >= MASK_BITS {
                *mask = u64::MAX;
            } else {
                let low = 1u64 << ((first >> shift) & MASK_BITS);
                let high = 1u64 << ((last >> shift) & MASK_BITS);
                *mask |= high
                    .wrapping_add(high.wrapping_sub(low))
                    .wrapping_sub(u64::from(high < low));
            }
        }
    }

    pub(crate) fn add_coverage(&mut self, coverage: &Coverage<'_>) {
        for (first, last) in coverage.ranges() {
            self.add_range(u32::from(first.0), u32::from(last.0));
        }
    }

    pub(crate) fn from_coverage(coverage: &Coverage<'_>) -> Self {
        let mut digest = Self::default();
        digest.add_coverage(coverage);
        digest
    }

    #[inline]
    pub(crate) fn union(&mut self, other: &Self) {
        for (mask, other) in self.masks.iter_mut().zip(other.masks) {
            *mask |= other;
        }
    }

    #[inline]
    pub(crate) fn may_have(&self, glyph: u32) -> bool {
        self.masks
            .iter()
            .zip(SHIFTS)
            .all(|(mask, shift)| mask & (1 << ((u64::from(glyph) >> shift) & MASK_BITS)) != 0)
    }

    #[inline]
    pub(crate) fn may_intersect(&self, other: &Self) -> bool {
        self.masks
            .iter()
            .zip(other.masks)
            .all(|(mask, other)| mask & other != 0)
    }
}
