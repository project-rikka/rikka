use std::collections::HashMap;
use std::hash::Hash;

use crate::error::{Error, Result};
use crate::ucd::CODEPOINT_COUNT;

const LEAF_BITS: std::ops::RangeInclusive<u32> = 2..=8;
const MID_BITS: std::ops::RangeInclusive<u32> = 2..=10;

#[derive(Debug)]
pub(crate) struct Trie<T> {
    pub(crate) leaf_bits: u32,
    pub(crate) mid_bits: u32,
    pub(crate) top: Vec<u16>,
    pub(crate) mid: Vec<u16>,
    pub(crate) leaf: Vec<T>,
}

impl<T: Copy + Eq + Hash> Trie<T> {
    pub(crate) fn smallest(values: &[T]) -> Result<Self> {
        let mut best: Option<Self> = None;
        for leaf_bits in LEAF_BITS {
            for mid_bits in MID_BITS {
                let Some(trie) = Self::build(values, leaf_bits, mid_bits) else {
                    continue;
                };
                if best.as_ref().is_none_or(|best| trie.bytes() < best.bytes()) {
                    best = Some(trie);
                }
            }
        }
        let trie = best.ok_or_else(|| Error::Data("no trie shape fits 16-bit offsets".into()))?;
        trie.verify(values)?;
        Ok(trie)
    }

    pub(crate) fn bytes(&self) -> usize {
        (self.top.len() + self.mid.len()) * 2 + self.leaf.len() * size_of::<T>()
    }

    fn build(values: &[T], leaf_bits: u32, mid_bits: u32) -> Option<Self> {
        let leaf_size = 1usize << leaf_bits;
        let mid_size = 1usize << mid_bits;
        let mut leaf = Vec::new();
        let mut known_leaves = HashMap::new();
        let leaf_offsets: Vec<u32> = values
            .chunks(leaf_size)
            .map(|block| append(&mut leaf, &mut known_leaves, block))
            .collect();
        let mut mid = Vec::new();
        let mut known_mids = HashMap::new();
        let top: Vec<u32> = leaf_offsets
            .chunks(mid_size)
            .map(|block| append(&mut mid, &mut known_mids, block))
            .collect();
        Some(Self {
            leaf_bits,
            mid_bits,
            top: narrow(&top)?,
            mid: narrow(&mid)?,
            leaf,
        })
    }

    fn get(&self, codepoint: usize) -> T {
        let top = self.top[codepoint >> (self.leaf_bits + self.mid_bits)] as usize;
        let mid = self.mid[top + ((codepoint >> self.leaf_bits) & ((1 << self.mid_bits) - 1))];
        self.leaf[mid as usize + (codepoint & ((1 << self.leaf_bits) - 1))]
    }

    fn verify(&self, values: &[T]) -> Result<()> {
        debug_assert_eq!(values.len(), CODEPOINT_COUNT);
        match (0..CODEPOINT_COUNT).find(|&codepoint| self.get(codepoint) != values[codepoint]) {
            Some(codepoint) => Err(Error::Data(format!(
                "trie lookup of U+{codepoint:04X} disagrees with the data"
            ))),
            None => Ok(()),
        }
    }
}

fn append<T: Copy + Eq + Hash>(
    store: &mut Vec<T>,
    known: &mut HashMap<Vec<T>, u32>,
    block: &[T],
) -> u32 {
    if let Some(&offset) = known.get(block) {
        return offset;
    }
    let overlap = (1..=block.len().min(store.len()))
        .rev()
        .find(|&length| store[store.len() - length..] == block[..length])
        .unwrap_or(0);
    let offset = (store.len() - overlap) as u32;
    store.extend_from_slice(&block[overlap..]);
    known.insert(block.to_vec(), offset);
    offset
}

fn narrow(offsets: &[u32]) -> Option<Vec<u16>> {
    offsets
        .iter()
        .map(|&offset| u16::try_from(offset).ok())
        .collect()
}
