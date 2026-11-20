use std::future::Future;
use std::num::NonZero;
use std::sync::Arc;
use std::thread;

use tokio::runtime::Handle;
use tokio::task::JoinHandle;

const MIN_CHUNK_LEN: usize = 8;

pub async fn offload<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    match Handle::try_current() {
        Ok(handle) => AbortOnDrop(handle.spawn_blocking(work)).join().await,
        Err(_) => work(),
    }
}

pub async fn detached<T: Send + 'static>(work: impl Future<Output = T> + Send + 'static) -> T {
    match Handle::try_current() {
        Ok(handle) => AbortOnDrop(handle.spawn(work)).join().await,
        Err(_) => work.await,
    }
}

pub async fn parallel_map<I, O>(
    items: Vec<I>,
    work: impl Fn(I) -> O + Send + Sync + 'static,
) -> Vec<O>
where
    I: Send + 'static,
    O: Send + 'static,
{
    map_in_chunks(items, MIN_CHUNK_LEN, work).await
}

pub async fn parallel_map_each<I, O>(
    items: Vec<I>,
    work: impl Fn(I) -> O + Send + Sync + 'static,
) -> Vec<O>
where
    I: Send + 'static,
    O: Send + 'static,
{
    map_in_chunks(items, 1, work).await
}

async fn map_in_chunks<I, O>(
    items: Vec<I>,
    min_chunk_len: usize,
    work: impl Fn(I) -> O + Send + Sync + 'static,
) -> Vec<O>
where
    I: Send + 'static,
    O: Send + 'static,
{
    let chunks = chunk_count(items.len(), min_chunk_len);
    if chunks <= 1 {
        return offload(move || items.into_iter().map(work).collect()).await;
    }
    let Ok(handle) = Handle::try_current() else {
        return items.into_iter().map(work).collect();
    };

    let work = Arc::new(work);
    let size = items.len().div_ceil(chunks);
    let mut items = items.into_iter();
    let mut tasks = Vec::with_capacity(chunks);
    loop {
        let chunk: Vec<I> = items.by_ref().take(size).collect();
        if chunk.is_empty() {
            break;
        }
        let work = Arc::clone(&work);
        tasks.push(AbortOnDrop(handle.spawn_blocking(move || {
            chunk.into_iter().map(|item| work(item)).collect::<Vec<O>>()
        })));
    }

    let mut collected = Vec::new();
    for chunk in tasks {
        collected.extend(chunk.join().await);
    }
    collected
}

fn chunk_count(count: usize, min_chunk_len: usize) -> usize {
    let cores = thread::available_parallelism().map_or(1, NonZero::get);
    count.div_ceil(min_chunk_len).min(cores)
}

struct AbortOnDrop<T>(JoinHandle<T>);

impl<T> AbortOnDrop<T> {
    async fn join(mut self) -> T {
        (&mut self.0)
            .await
            .unwrap_or_else(|failure| std::panic::resume_unwind(failure.into_panic()))
    }
}

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}
