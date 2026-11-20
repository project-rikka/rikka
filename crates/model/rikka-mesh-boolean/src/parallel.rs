use std::num::NonZeroUsize;
use std::ops::Range;

const THREAD_NAME: &str = "rikka-mesh-boolean";

pub(crate) fn thread_count(requested: Option<NonZeroUsize>) -> usize {
    requested
        .or_else(|| std::thread::available_parallelism().ok())
        .map_or(1, NonZeroUsize::get)
}

pub(crate) fn map_chunks<R, F>(len: usize, threads: usize, minimum: usize, work: F) -> Vec<R>
where
    R: Send,
    F: Fn(Range<usize>) -> Vec<R> + Sync,
{
    let threads = threads.min(len / minimum.max(1)).max(1);
    if threads <= 1 {
        return work(0..len);
    }
    let chunk = len.div_ceil(threads);
    let ranges: Vec<Range<usize>> = (0..threads)
        .map(|index| (index * chunk).min(len)..((index + 1) * chunk).min(len))
        .filter(|range| !range.is_empty())
        .collect();
    let work = &work;
    std::thread::scope(|scope| {
        let handles: Vec<_> = ranges
            .into_iter()
            .map(|range| {
                std::thread::Builder::new()
                    .name(THREAD_NAME.to_owned())
                    .spawn_scoped(scope, move || work(range))
                    .expect("failed to spawn rikka-mesh-boolean worker")
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("rikka-mesh-boolean worker panicked"))
            .collect()
    })
}
