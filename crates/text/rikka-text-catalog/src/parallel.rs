use std::num::NonZero;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

const THREAD_NAME: &str = "rikka-font-scan";

pub(crate) fn parallel_map<T, R, S>(
    items: &[T],
    state: impl Fn() -> S + Sync,
    work: impl Fn(&mut S, &T) -> R + Sync,
) -> Vec<R>
where
    T: Sync,
    R: Send,
{
    let workers = thread::available_parallelism()
        .map_or(1, NonZero::get)
        .min(items.len())
        .saturating_sub(1);
    let next = AtomicUsize::new(0);
    let drain = || {
        let mut state = state();
        let mut done = Vec::new();
        loop {
            let index = next.fetch_add(1, Ordering::Relaxed);
            let Some(item) = items.get(index) else {
                break done;
            };
            done.push((index, work(&mut state, item)));
        }
    };
    let batches: Vec<Vec<(usize, R)>> = thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .filter_map(|_| {
                thread::Builder::new()
                    .name(THREAD_NAME.to_owned())
                    .spawn_scoped(scope, drain)
                    .ok()
            })
            .collect();
        let mut batches = vec![drain()];
        for handle in handles {
            match handle.join() {
                Ok(batch) => batches.push(batch),
                Err(panic) => std::panic::resume_unwind(panic),
            }
        }
        batches
    });
    let mut slots: Vec<Option<R>> = std::iter::repeat_with(|| None).take(items.len()).collect();
    for (index, result) in batches.into_iter().flatten() {
        slots[index] = Some(result);
    }
    slots.into_iter().flatten().collect()
}
