#[derive(Debug)]
pub(crate) struct Trie<T: 'static, const LEAF_BITS: u32, const MID_BITS: u32> {
    top: &'static [u16],
    mid: &'static [u16],
    leaf: &'static [T],
}

impl<T: Copy, const LEAF_BITS: u32, const MID_BITS: u32> Trie<T, LEAF_BITS, MID_BITS> {
    pub(crate) const fn new(top: &'static [u16], mid: &'static [u16], leaf: &'static [T]) -> Self {
        Self { top, mid, leaf }
    }

    #[inline(always)]
    pub(crate) fn get(&self, codepoint: u32) -> T {
        let top = usize::from(self.top[(codepoint >> (LEAF_BITS + MID_BITS)) as usize]);
        let mid = top + ((codepoint >> LEAF_BITS) & ((1 << MID_BITS) - 1)) as usize;
        let leaf = usize::from(self.mid[mid]) + (codepoint & ((1 << LEAF_BITS) - 1)) as usize;
        self.leaf[leaf]
    }
}
