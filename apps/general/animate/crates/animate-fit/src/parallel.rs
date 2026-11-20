use std::thread;

pub(crate) fn across<I: Sync, T: Send>(
    items: &[I],
    shortest: usize,
    each: impl Fn(&I) -> T + Sync,
) -> Vec<T> {
    let workers = thread::available_parallelism().map_or(1, usize::from);
    let share = items.len().div_ceil(workers).max(shortest);
    thread::scope(|scope| {
        let each = &each;
        let handles: Vec<_> = items
            .chunks(share)
            .enumerate()
            .map(|(at, chunk)| {
                thread::Builder::new()
                    .name(format!("rikka-fit-{at}"))
                    .spawn_scoped(scope, move || chunk.iter().map(each).collect::<Vec<T>>())
                    .expect("failed to spawn a fitting thread")
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    })
}
