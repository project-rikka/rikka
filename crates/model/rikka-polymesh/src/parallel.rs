use std::cell::Cell;
use std::num::NonZeroUsize;
use std::ops::Range;
use std::thread;

const THREAD_NAME: &str = "rikka-polymesh";

thread_local! {
    static INSIDE: Cell<bool> = const { Cell::new(false) };
}

fn workers(len: usize, grain: usize) -> usize {
    if INSIDE.get() {
        return 1;
    }
    thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(len / grain.max(1))
        .max(1)
}

fn inside<R>(work: impl FnOnce() -> R) -> R {
    let outer = INSIDE.replace(true);
    let result = work();
    INSIDE.set(outer);
    result
}

fn ranges(len: usize, grain: usize) -> Vec<Range<usize>> {
    let workers = workers(len, grain);
    let chunk = len.div_ceil(workers).max(1);
    (0..workers)
        .map(|index| (index * chunk).min(len)..((index + 1) * chunk).min(len))
        .filter(|range| !range.is_empty() || len == 0)
        .collect()
}

pub(crate) fn map_chunks<R: Send>(
    len: usize,
    grain: usize,
    work: impl Fn(Range<usize>) -> R + Sync,
) -> Vec<R> {
    let ranges = ranges(len, grain);
    match ranges.as_slice() {
        [] => Vec::new(),
        [only] => vec![work(only.clone())],
        [first, rest @ ..] => {
            let work = &work;
            thread::scope(|scope| {
                let spawned: Vec<_> = rest
                    .iter()
                    .map(|range| {
                        let range = range.clone();
                        thread::Builder::new()
                            .name(THREAD_NAME.into())
                            .spawn_scoped(scope, move || inside(|| work(range)))
                            .expect("failed to spawn rikka-polymesh worker")
                    })
                    .collect();
                let mut results = Vec::with_capacity(ranges.len());
                results.push(inside(|| work(first.clone())));
                for handle in spawned {
                    results.push(
                        handle
                            .join()
                            .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
                    );
                }
                results
            })
        }
    }
}

pub(crate) fn collect<T: Send>(
    len: usize,
    grain: usize,
    work: impl Fn(usize) -> T + Sync,
) -> Vec<T> {
    let parts = map_chunks(len, grain, |range| range.map(&work).collect::<Vec<T>>());
    match parts.len() {
        1 => parts.into_iter().next().expect("one part"),
        _ => {
            let mut output = Vec::with_capacity(len);
            for part in parts {
                output.extend(part);
            }
            output
        }
    }
}

pub(crate) fn fill<T: Send>(output: &mut [T], grain: usize, work: impl Fn(usize, &mut T) + Sync) {
    let ranges = ranges(output.len(), grain);
    let mut pieces: Vec<(Range<usize>, &mut [T])> = Vec::with_capacity(ranges.len());
    let mut rest = output;
    for range in &ranges {
        let (piece, tail) = rest.split_at_mut(range.len());
        pieces.push((range.clone(), piece));
        rest = tail;
    }
    let work = &work;
    let run = move |range: Range<usize>, piece: &mut [T]| {
        for (index, slot) in range.zip(piece) {
            work(index, slot);
        }
    };
    let mut pieces = pieces.into_iter();
    let Some((first_range, first)) = pieces.next() else {
        return;
    };
    thread::scope(|scope| {
        let spawned: Vec<_> = pieces
            .map(|(range, piece)| {
                thread::Builder::new()
                    .name(THREAD_NAME.into())
                    .spawn_scoped(scope, move || inside(|| run(range, piece)))
                    .expect("failed to spawn rikka-polymesh worker")
            })
            .collect();
        inside(|| run(first_range, first));
        for handle in spawned {
            handle
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
        }
    });
}

pub(crate) fn gather<T: Copy + Send + Sync>(source: &[T], indices: &[u32]) -> Vec<T> {
    let Some(&seed) = source.first() else {
        return indices
            .iter()
            .map(|&index| source[index as usize])
            .collect();
    };
    let mut output = vec![seed; indices.len()];
    fill(&mut output, 1 << 14, |index, slot| {
        *slot = source[indices[index] as usize]
    });
    output
}
