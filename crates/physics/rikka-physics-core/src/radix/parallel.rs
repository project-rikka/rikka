use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

use super::{BUCKETS, MAX_PASSES, RadixKey, digit, passes};
use crate::{Block, Executor, Shared, Stage};

const MIN_BLOCK: usize = 1 << 12;
const BLOCKS_PER_WORKER: usize = 4;
const PHASES: usize = 3;

#[derive(Clone, Copy)]
enum Op {
    Histogram { pass: usize, from_scratch: bool },
    Prefix,
    Scatter { pass: usize, to_scratch: bool },
}

struct Buffers<'a, T> {
    items: Shared<'a, T>,
    scratch: Shared<'a, T>,
}

impl<'a, T> Buffers<'a, T> {
    #[inline]
    fn slice(&self, in_scratch: bool) -> Shared<'a, T> {
        if in_scratch { self.scratch } else { self.items }
    }
}

struct Plan<'a, T, F> {
    buffers: Buffers<'a, T>,
    key: &'a F,
    counts: &'a [AtomicU32],
    bucket_offsets: &'a [AtomicU32],
    or_bits: AtomicU64,
    and_bits: AtomicU64,
    blocks: usize,
    block_width: usize,
}

pub(super) fn sort<T: Copy + Send + Sync, K: RadixKey, F: Fn(&T) -> K + Sync>(
    block_counts: &mut Vec<AtomicU32>,
    executor: &(impl Executor + ?Sized),
    items: &mut [T],
    scratch: &mut [T],
    key: &F,
) {
    let len = items.len();
    let blocks = len
        .div_ceil(MIN_BLOCK)
        .min(executor.workers() * BLOCKS_PER_WORKER)
        .max(1);
    let block_width = len.div_ceil(blocks);
    let blocks = len.div_ceil(block_width);
    let needed = (blocks + 1) * BUCKETS;
    if block_counts.len() < needed {
        block_counts.resize_with(needed, AtomicU32::default);
    }
    let (counts, bucket_offsets) = block_counts[..needed].split_at(blocks * BUCKETS);
    let plan = Plan {
        buffers: Buffers {
            items: Shared::new(items),
            scratch: Shared::new(scratch),
        },
        key,
        counts,
        bucket_offsets,
        or_bits: AtomicU64::new(0),
        and_bits: AtomicU64::new(u64::MAX),
        blocks,
        block_width,
    };

    executor.run(&[Stage::new("radix histogram", blocks as u32)], &|block| {
        plan.histogram_first(block)
    });

    let varying = plan.or_bits.load(Relaxed) ^ plan.and_bits.load(Relaxed);
    let passes = passes::<K>();
    let mut active = [0usize; MAX_PASSES];
    let mut active_count = 0;
    for pass in 0..passes {
        if digit(varying, pass) != 0 {
            active[active_count] = pass;
            active_count += 1;
        }
    }
    if active_count == 0 {
        return;
    }

    let mut stages = [Stage::new("", 0); MAX_PASSES * PHASES];
    let mut ops = [Op::Prefix; MAX_PASSES * PHASES];
    let mut count = 0;
    let mut sorted_in_scratch = false;
    let start = if active[0] == 0 {
        plan.prefix();
        stages[count] = Stage::new("radix scatter", blocks as u32);
        ops[count] = Op::Scatter {
            pass: 0,
            to_scratch: true,
        };
        count += 1;
        sorted_in_scratch = true;
        1
    } else {
        0
    };
    for &pass in &active[start..active_count] {
        stages[count] = Stage::new("radix histogram", blocks as u32);
        ops[count] = Op::Histogram {
            pass,
            from_scratch: sorted_in_scratch,
        };
        count += 1;
        stages[count] = Stage::new("radix prefix", 1);
        ops[count] = Op::Prefix;
        count += 1;
        stages[count] = Stage::new("radix scatter", blocks as u32);
        ops[count] = Op::Scatter {
            pass,
            to_scratch: !sorted_in_scratch,
        };
        count += 1;
        sorted_in_scratch = !sorted_in_scratch;
    }
    executor.run(&stages[..count], &|block| {
        plan.execute(ops[block.stage as usize], block);
    });
    if sorted_in_scratch {
        items.copy_from_slice(scratch);
    }
}

impl<'a, T: Copy, K: RadixKey, F: Fn(&T) -> K> Plan<'a, T, F> {
    fn execute(&self, op: Op, block: Block) {
        match op {
            Op::Histogram { pass, from_scratch } => {
                self.histogram(self.buffers.slice(from_scratch), pass, block);
            }
            Op::Prefix => self.prefix(),
            Op::Scatter { pass, to_scratch } => {
                let source = self.buffers.slice(!to_scratch);
                let target = self.buffers.slice(to_scratch);
                self.scatter(source, target, pass, block);
            }
        }
    }

    #[inline]
    fn row(&self, block: usize) -> &[AtomicU32] {
        &self.counts[block * BUCKETS..][..BUCKETS]
    }

    #[inline]
    fn items(&self, source: Shared<'a, T>, block: Block) -> &[T] {
        let range = block.range(self.block_width, source.len());
        unsafe { source.slice(range.start, range.end) }
    }

    fn histogram_first(&self, block: Block) {
        let mut histogram = [0u32; BUCKETS];
        let mut or_acc = 0u64;
        let mut and_acc = u64::MAX;
        for item in self.items(self.buffers.items, block) {
            let bits = (self.key)(item).radix_bits();
            or_acc |= bits;
            and_acc &= bits;
            histogram[digit(bits, 0)] += 1;
        }
        self.or_bits.fetch_or(or_acc, Relaxed);
        self.and_bits.fetch_and(and_acc, Relaxed);
        for (count, &value) in self.row(block.index as usize).iter().zip(&histogram) {
            count.store(value, Relaxed);
        }
    }

    fn histogram(&self, source: Shared<'a, T>, pass: usize, block: Block) {
        let mut histogram = [0u32; BUCKETS];
        for item in self.items(source, block) {
            histogram[digit((self.key)(item).radix_bits(), pass)] += 1;
        }
        for (count, &value) in self.row(block.index as usize).iter().zip(&histogram) {
            count.store(value, Relaxed);
        }
    }

    fn prefix(&self) {
        let mut totals = [0u32; BUCKETS];
        for block in 0..self.blocks {
            for (total, count) in totals.iter_mut().zip(self.row(block)) {
                let value = count.load(Relaxed);
                count.store(*total, Relaxed);
                *total += value;
            }
        }
        let mut running = 0u32;
        for (offset, &total) in self.bucket_offsets.iter().zip(&totals) {
            offset.store(running, Relaxed);
            running += total;
        }
    }

    fn scatter(&self, source: Shared<'a, T>, target: Shared<'a, T>, pass: usize, block: Block) {
        let index = block.index as usize;
        let row = self.row(index);
        let next_row = (index + 1 < self.blocks).then(|| self.row(index + 1));
        let mut next = [0u32; BUCKETS];
        let mut end = [0u32; BUCKETS];
        for bucket in 0..BUCKETS {
            let global = self.bucket_offsets[bucket].load(Relaxed);
            next[bucket] = global + row[bucket].load(Relaxed);
            end[bucket] = if let Some(following) = next_row {
                global + following[bucket].load(Relaxed)
            } else if bucket + 1 < BUCKETS {
                self.bucket_offsets[bucket + 1].load(Relaxed)
            } else {
                target.len() as u32
            };
        }
        for item in self.items(source, block) {
            let bucket = digit((self.key)(item).radix_bits(), pass);
            let slot = next[bucket];
            assert!(slot < end[bucket], "radix sort key changed while sorting");
            unsafe { target.write(slot as usize, *item) };
            next[bucket] = slot + 1;
        }
    }
}
