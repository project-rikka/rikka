use std::ops::Range;
use std::panic;
use std::thread::{self, Scope, ScopedJoinHandle};

use rikka_sync::Mutex;

use crate::Threads;
use crate::nesting::{inside, is_inside};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parallel {
    name: &'static str,
    threads: Option<Threads>,
    grain: usize,
}

impl Parallel {
    #[must_use]
    pub const fn new(name: &'static str) -> Parallel {
        Parallel {
            name,
            threads: None,
            grain: 1,
        }
    }

    #[must_use]
    pub const fn threads(self, threads: Threads) -> Parallel {
        Parallel {
            threads: Some(threads),
            ..self
        }
    }

    #[must_use]
    pub const fn grain(self, grain: usize) -> Parallel {
        Parallel {
            grain: if grain == 0 { 1 } else { grain },
            ..self
        }
    }

    #[must_use]
    pub fn ranges(&self, len: usize) -> Vec<Range<usize>> {
        let workers = self.workers(len);
        let chunk = len.div_ceil(workers).max(1);
        (0..workers)
            .map(|index| (index * chunk).min(len)..((index + 1) * chunk).min(len))
            .filter(|range| !range.is_empty() || len == 0)
            .collect()
    }

    pub fn map_ranges<R: Send>(
        &self,
        ranges: &[Range<usize>],
        work: impl Fn(Range<usize>) -> R + Sync,
    ) -> Vec<R> {
        match ranges {
            [] => Vec::new(),
            [only] => vec![work(only.clone())],
            [first, rest @ ..] => {
                let work = &work;
                thread::scope(|scope| {
                    let spawned: Vec<_> = rest
                        .iter()
                        .map(|range| {
                            let range = range.clone();
                            self.spawn(scope, move || work(range))
                        })
                        .collect();
                    let mut results = Vec::with_capacity(ranges.len());
                    results.push(inside(|| work(first.clone())));
                    results.extend(spawned.into_iter().map(join));
                    results
                })
            }
        }
    }

    pub fn map_chunks<R: Send>(
        &self,
        len: usize,
        work: impl Fn(Range<usize>) -> R + Sync,
    ) -> Vec<R> {
        self.map_ranges(&self.ranges(len), work)
    }

    pub fn concat_chunks<T: Send>(
        &self,
        len: usize,
        work: impl Fn(Range<usize>) -> Vec<T> + Sync,
    ) -> Vec<T> {
        let parts = self.map_chunks(len, work);
        match parts.len() {
            1 => parts.into_iter().next().expect("one part"),
            _ => {
                let mut output = Vec::with_capacity(parts.iter().map(Vec::len).sum());
                for part in parts {
                    output.extend(part);
                }
                output
            }
        }
    }

    pub fn collect<T: Send>(&self, len: usize, work: impl Fn(usize) -> T + Sync) -> Vec<T> {
        self.concat_chunks(len, |range| range.map(&work).collect())
    }

    pub fn split_mut<T: Send>(
        &self,
        output: &mut [T],
        ranges: &[Range<usize>],
        work: impl Fn(usize, &mut [T]) + Sync,
    ) {
        let mut pieces: Vec<(usize, &mut [T])> = Vec::with_capacity(ranges.len());
        let mut rest = output;
        for (index, range) in ranges.iter().enumerate() {
            let (piece, tail) = rest.split_at_mut(range.len());
            pieces.push((index, piece));
            rest = tail;
        }
        let work = &work;
        let mut pieces = pieces.into_iter();
        let Some((first_index, first)) = pieces.next() else {
            return;
        };
        if pieces.len() == 0 {
            work(first_index, first);
            return;
        }
        thread::scope(|scope| {
            let spawned: Vec<_> = pieces
                .map(|(index, piece)| self.spawn(scope, move || work(index, piece)))
                .collect();
            inside(|| work(first_index, first));
            spawned.into_iter().for_each(join);
        });
    }

    pub fn fill<T: Send>(&self, output: &mut [T], work: impl Fn(usize, &mut T) + Sync) {
        let ranges = self.ranges(output.len());
        self.split_mut(output, &ranges, |chunk, piece| {
            for (index, slot) in ranges[chunk].clone().zip(piece) {
                work(index, slot);
            }
        });
    }

    pub fn map<I, R>(&self, items: I, work: impl Fn(I::Item) -> R + Sync) -> Vec<R>
    where
        I: IntoIterator,
        I::IntoIter: ExactSizeIterator + Send,
        R: Send,
    {
        self.map_with(items, || (), |(), item| work(item))
    }

    pub fn map_with<I, S, R>(
        &self,
        items: I,
        state: impl Fn() -> S + Sync,
        work: impl Fn(&mut S, I::Item) -> R + Sync,
    ) -> Vec<R>
    where
        I: IntoIterator,
        I::IntoIter: ExactSizeIterator + Send,
        R: Send,
    {
        let items = items.into_iter();
        let len = items.len();
        let workers = self.workers(len);
        if workers == 1 {
            let mut state = state();
            return items.map(|item| work(&mut state, item)).collect();
        }
        let queue = Mutex::new(items.enumerate());
        let drain = || {
            let mut state = state();
            let mut done = Vec::new();
            loop {
                let claimed = queue.lock().next();
                let Some((index, item)) = claimed else {
                    break done;
                };
                done.push((index, work(&mut state, item)));
            }
        };
        let drain = &drain;
        let batches: Vec<Vec<(usize, R)>> = thread::scope(|scope| {
            let spawned: Vec<_> = (1..workers).map(|_| self.spawn(scope, drain)).collect();
            let mut batches = Vec::with_capacity(workers);
            batches.push(inside(drain));
            batches.extend(spawned.into_iter().map(join));
            batches
        });
        let mut slots: Vec<Option<R>> = std::iter::repeat_with(|| None).take(len).collect();
        for (index, result) in batches.into_iter().flatten() {
            slots[index] = Some(result);
        }
        slots
            .into_iter()
            .map(|slot| slot.expect("every item ran"))
            .collect()
    }

    fn workers(&self, len: usize) -> usize {
        if is_inside() {
            return 1;
        }
        let threads = self.threads.unwrap_or_else(Threads::available);
        threads.get().min(len / self.grain).max(1)
    }

    fn spawn<'scope, T: Send + 'scope>(
        &self,
        scope: &'scope Scope<'scope, '_>,
        work: impl FnOnce() -> T + Send + 'scope,
    ) -> ScopedJoinHandle<'scope, T> {
        let name = self.name;
        thread::Builder::new()
            .name(name.into())
            .spawn_scoped(scope, move || inside(work))
            .unwrap_or_else(|error| panic!("failed to spawn {name} worker: {error}"))
    }
}

fn join<T>(handle: ScopedJoinHandle<'_, T>) -> T {
    handle
        .join()
        .unwrap_or_else(|payload| panic::resume_unwind(payload))
}
