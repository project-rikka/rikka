mod parallel;
mod serial;

use std::sync::atomic::AtomicU32;

use crate::Executor;

const DIGIT_BITS: u32 = 11;
const BUCKETS: usize = 1 << DIGIT_BITS;
const MAX_PASSES: usize = 64usize.div_ceil(DIGIT_BITS as usize);
const INSERTION_LIMIT: usize = 48;
const PARALLEL_LIMIT: usize = 1 << 15;

pub trait RadixKey: Copy {
    const BITS: u32;

    fn radix_bits(self) -> u64;
}

impl RadixKey for u32 {
    const BITS: u32 = 32;

    #[inline]
    fn radix_bits(self) -> u64 {
        u64::from(self)
    }
}

impl RadixKey for u64 {
    const BITS: u32 = 64;

    #[inline]
    fn radix_bits(self) -> u64 {
        self
    }
}

#[inline]
fn passes<K: RadixKey>() -> usize {
    K::BITS.div_ceil(DIGIT_BITS) as usize
}

#[inline]
fn digit(bits: u64, pass: usize) -> usize {
    ((bits >> (pass as u32 * DIGIT_BITS)) as usize) & (BUCKETS - 1)
}

#[derive(Debug, Default)]
pub struct RadixSorter {
    counts: Vec<u32>,
    block_counts: Vec<AtomicU32>,
}

impl RadixSorter {
    pub const fn new() -> Self {
        Self {
            counts: Vec::new(),
            block_counts: Vec::new(),
        }
    }

    pub fn sort<T: Copy, K: RadixKey>(
        &mut self,
        items: &mut [T],
        scratch: &mut [T],
        key: impl Fn(&T) -> K,
    ) {
        let scratch = checked_scratch(items, scratch);
        if items.len() <= INSERTION_LIMIT {
            insertion_sort(items, &key);
        } else {
            serial::sort(&mut self.counts, items, scratch, &key);
        }
    }

    pub fn par_sort<T: Copy + Send + Sync, K: RadixKey>(
        &mut self,
        executor: &(impl Executor + ?Sized),
        items: &mut [T],
        scratch: &mut [T],
        key: impl Fn(&T) -> K + Sync,
    ) {
        if items.len() < PARALLEL_LIMIT || executor.workers() == 1 {
            self.sort(items, scratch, key);
            return;
        }
        let scratch = checked_scratch(items, scratch);
        parallel::sort(&mut self.block_counts, executor, items, scratch, &key);
    }
}

fn checked_scratch<'a, T>(items: &[T], scratch: &'a mut [T]) -> &'a mut [T] {
    assert!(
        u32::try_from(items.len()).is_ok(),
        "radix sort handles at most u32::MAX items"
    );
    assert!(
        scratch.len() >= items.len(),
        "radix sort scratch holds {} items but {} are sorted",
        scratch.len(),
        items.len()
    );
    &mut scratch[..items.len()]
}

fn insertion_sort<T: Copy, K: RadixKey>(items: &mut [T], key: &impl Fn(&T) -> K) {
    for next in 1..items.len() {
        let item = items[next];
        let bits = key(&item).radix_bits();
        let mut hole = next;
        while hole > 0 && key(&items[hole - 1]).radix_bits() > bits {
            items[hole] = items[hole - 1];
            hole -= 1;
        }
        items[hole] = item;
    }
}
