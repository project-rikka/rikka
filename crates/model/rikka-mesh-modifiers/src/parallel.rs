use std::cell::Cell;
use std::num::NonZeroUsize;
use std::ops::Range;
use std::thread;

const THREAD_NAME: &str = "rikka-mesh-modifiers";

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

pub(crate) fn ranges(len: usize, grain: usize) -> Vec<Range<usize>> {
    let workers = workers(len, grain);
    let chunk = len.div_ceil(workers).max(1);
    (0..workers)
        .map(|index| (index * chunk).min(len)..((index + 1) * chunk).min(len))
        .filter(|range| !range.is_empty() || len == 0)
        .take(workers)
        .collect()
}

pub(crate) fn map_ranges<R: Send>(
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
                        thread::Builder::new()
                            .name(THREAD_NAME.into())
                            .spawn_scoped(scope, move || inside(|| work(range)))
                            .expect("failed to spawn rikka-mesh-modifiers worker")
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

pub(crate) fn map_chunks<R: Send>(
    len: usize,
    grain: usize,
    work: impl Fn(Range<usize>) -> R + Sync,
) -> Vec<R> {
    map_ranges(&ranges(len, grain), work)
}

pub(crate) fn split_mut<T: Send>(
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
    match pieces.len() {
        0 => {}
        1 => {
            let (index, piece) = pieces.pop().expect("one piece");
            work(index, piece);
        }
        _ => thread::scope(|scope| {
            let mut pieces = pieces.into_iter();
            let (first_index, first) = pieces.next().expect("several pieces");
            let spawned: Vec<_> = pieces
                .map(|(index, piece)| {
                    thread::Builder::new()
                        .name(THREAD_NAME.into())
                        .spawn_scoped(scope, move || inside(|| work(index, piece)))
                        .expect("failed to spawn rikka-mesh-modifiers worker")
                })
                .collect();
            inside(|| work(first_index, first));
            for handle in spawned {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            }
        }),
    }
}

pub(crate) fn fill<T: Send>(output: &mut [T], grain: usize, work: impl Fn(usize, &mut T) + Sync) {
    let ranges = ranges(output.len(), grain);
    split_mut(output, &ranges, |chunk, piece| {
        for (index, slot) in ranges[chunk].clone().zip(piece) {
            work(index, slot);
        }
    });
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
