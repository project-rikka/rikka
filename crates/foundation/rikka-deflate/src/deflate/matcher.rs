use crate::tables::{MAX_MATCH, MIN_MATCH, WINDOW};

const HASH_BITS: u32 = 15;
const HASH_SIZE: usize = 1 << HASH_BITS;
const HASH_MULTIPLIER: u32 = 0x9E37_79B1;
const WINDOW_MASK: usize = WINDOW - 1;
const UNSET: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Match {
    pub(crate) length: usize,
    pub(crate) distance: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Search {
    pub(crate) chain: usize,
    pub(crate) nice: usize,
    pub(crate) longer_than: usize,
}

#[derive(Debug)]
pub(crate) struct Matcher {
    head: Vec<u32>,
    previous: Vec<u32>,
}

impl Matcher {
    pub(crate) fn new() -> Self {
        Self {
            head: vec![UNSET; HASH_SIZE],
            previous: vec![UNSET; WINDOW],
        }
    }

    pub(crate) fn reset(&mut self) {
        self.head.fill(UNSET);
    }

    #[inline]
    pub(crate) fn insert(&mut self, data: &[u8], position: usize) {
        let Some(slot) = slot(data, position) else {
            return;
        };
        self.previous[position & WINDOW_MASK] = self.head[slot];
        self.head[slot] = position as u32;
    }

    #[inline]
    pub(crate) fn longest(&self, data: &[u8], position: usize, search: Search) -> Option<Match> {
        let slot = slot(data, position)?;
        let ahead = &data[position..];
        let ahead = &ahead[..ahead.len().min(MAX_MATCH)];
        let mut best = Match {
            length: search.longer_than,
            distance: 0,
        };
        let mut candidate = self.head[slot];
        let mut reach = 0;
        for _ in 0..search.chain {
            if best.length >= ahead.len() {
                break;
            }
            let distance = (position as u32).wrapping_sub(candidate) as usize;
            if distance <= reach || distance > WINDOW || distance > position {
                break;
            }
            reach = distance;
            let start = position - distance;
            if data[start + best.length] == ahead[best.length] {
                let length = common_prefix(&data[start..start + ahead.len()], ahead);
                if length > best.length {
                    best = Match { length, distance };
                    if length >= search.nice {
                        break;
                    }
                }
            }
            candidate = self.previous[start & WINDOW_MASK];
        }
        (best.distance != 0 && best.length >= MIN_MATCH).then_some(best)
    }
}

#[inline]
fn slot(data: &[u8], position: usize) -> Option<usize> {
    let bytes = data.get(position..position + MIN_MATCH)?;
    let word = u32::from(bytes[0]) | u32::from(bytes[1]) << 8 | u32::from(bytes[2]) << 16;
    Some((word.wrapping_mul(HASH_MULTIPLIER) >> (32 - HASH_BITS)) as usize)
}

#[inline]
fn common_prefix(behind: &[u8], ahead: &[u8]) -> usize {
    let (behind_words, behind_tail) = behind.as_chunks::<8>();
    let (ahead_words, ahead_tail) = ahead.as_chunks::<8>();
    for (index, (left, right)) in behind_words.iter().zip(ahead_words).enumerate() {
        let difference = u64::from_le_bytes(*left) ^ u64::from_le_bytes(*right);
        if difference != 0 {
            return index * 8 + (difference.trailing_zeros() / 8) as usize;
        }
    }
    let equal = behind_tail
        .iter()
        .zip(ahead_tail)
        .take_while(|(left, right)| left == right)
        .count();
    ahead_words.len() * 8 + equal
}
