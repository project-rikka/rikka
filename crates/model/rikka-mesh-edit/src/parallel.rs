use std::num::NonZeroUsize;
use std::ops::Range;
use std::thread;

const SERIAL_LIMIT: usize = 1 << 14;

fn ranges<T: Send>(len: usize, run: impl Fn(Range<usize>) -> T + Sync) -> Vec<T> {
    let workers = thread::available_parallelism()
        .map(NonZeroUsize::get)
        .unwrap_or(1)
        .min(len / SERIAL_LIMIT)
        .max(1);
    if workers == 1 {
        return vec![run(0..len)];
    }
    let chunk = len.div_ceil(workers);
    let run = &run;
    thread::scope(|scope| {
        let mut parts: Vec<PartState<'_, T>> = Vec::with_capacity(workers);
        for start in (0..len).step_by(chunk) {
            let range = start..(start + chunk).min(len);
            let spawned = thread::Builder::new()
                .name("rikka-mesh-edit".into())
                .spawn_scoped(scope, {
                    let range = range.clone();
                    move || run(range)
                });
            parts.push(match spawned {
                Ok(handle) => PartState::Running(handle),
                Err(_) => PartState::Inline(range),
            });
        }
        parts
            .into_iter()
            .map(|part| match part {
                PartState::Running(handle) => match handle.join() {
                    Ok(values) => values,
                    Err(panic) => std::panic::resume_unwind(panic),
                },
                PartState::Inline(range) => run(range),
            })
            .collect()
    })
}

pub(crate) fn collect<T: Send>(len: usize, compute: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let parts = ranges(len, |range| range.map(&compute).collect::<Vec<T>>());
    match parts.len() {
        1 => parts.into_iter().next().unwrap_or_default(),
        _ => {
            let mut output = Vec::with_capacity(len);
            for part in parts {
                output.extend(part);
            }
            output
        }
    }
}

pub(crate) fn concat<T: Send>(len: usize, run: impl Fn(Range<usize>) -> Vec<T> + Sync) -> Vec<T> {
    let parts = ranges(len, run);
    let mut output = Vec::with_capacity(parts.iter().map(Vec::len).sum());
    for part in parts {
        output.extend(part);
    }
    output
}

pub(crate) fn indices_where(len: usize, test: impl Fn(usize) -> bool + Sync) -> Vec<u32> {
    ranges(len, |range| {
        range
            .filter(|&index| test(index))
            .map(|index| index as u32)
            .collect::<Vec<u32>>()
    })
    .concat()
}

enum PartState<'scope, T> {
    Running(thread::ScopedJoinHandle<'scope, T>),
    Inline(Range<usize>),
}
