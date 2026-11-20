mod pool;
mod serial;
mod workers;

use std::ops::Range;

use crate::Shared;

pub use pool::{WorkerPool, WorkerPoolBuilder};
pub use serial::SerialExecutor;
pub use workers::{Threads, Workers};

type ThreadName = Box<dyn Fn(usize) -> String>;

pub trait Executor: Send + Sync {
    fn workers(&self) -> usize;

    fn run(&self, stages: &[Stage], task: &(dyn Fn(Block) + Sync));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stage {
    pub name: &'static str,
    pub blocks: u32,
}

impl Stage {
    pub const fn new(name: &'static str, blocks: u32) -> Self {
        Self { name, blocks }
    }

    pub fn for_items(name: &'static str, items: usize, block_width: usize) -> Self {
        assert!(block_width > 0, "stage {name} needs a positive block width");
        let blocks = u32::try_from(items.div_ceil(block_width))
            .unwrap_or_else(|_| panic!("stage {name} has more than u32::MAX blocks"));
        Self { name, blocks }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub stage: u32,
    pub index: u32,
    pub worker: u32,
}

impl Block {
    #[inline]
    pub fn range(self, block_width: usize, items: usize) -> Range<usize> {
        let start = (self.index as usize * block_width).min(items);
        start..(start + block_width).min(items)
    }
}

pub trait ExecutorExt: Executor {
    fn for_each(&self, blocks: u32, task: impl Fn(Block) + Sync) {
        self.run(&[Stage::new("for_each", blocks)], &task);
    }

    fn for_each_item<T: Send>(
        &self,
        name: &'static str,
        items: &mut [T],
        task: impl Fn(usize, &mut T) + Sync,
    ) {
        self.for_each_chunk(name, items, 1, |start, chunk| {
            task(start, &mut chunk[0]);
        });
    }

    fn for_each_chunk<T: Send>(
        &self,
        name: &'static str,
        items: &mut [T],
        width: usize,
        task: impl Fn(usize, &mut [T]) + Sync,
    ) {
        let count = items.len();
        let shared = Shared::new(items);
        self.run(&[Stage::for_items(name, count, width)], &|block| {
            let range = block.range(width, count);
            let chunk = unsafe { shared.slice(range.start, range.end) };
            task(range.start, chunk);
        });
    }
}

impl<E: Executor + ?Sized> ExecutorExt for E {}

pub(crate) fn run_serially(stages: &[Stage], task: &(dyn Fn(Block) + Sync), worker: u32) {
    for (stage, blocks) in (0u32..).zip(stages.iter().map(|stage| stage.blocks)) {
        for index in 0..blocks {
            task(Block {
                stage,
                index,
                worker,
            });
        }
    }
}
