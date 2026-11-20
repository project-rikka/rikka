use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;
use std::task::{Context, Poll};

use music_core::{Error, Result};
use tokio::runtime::{Builder, Runtime};
use tokio::task::{JoinError, JoinHandle};

static WORKER: OnceLock<Runtime> = OnceLock::new();

const WORKERS: usize = 2;

fn runtime() -> &'static Runtime {
    WORKER.get_or_init(|| {
        Builder::new_multi_thread()
            .worker_threads(WORKERS)
            .thread_name("rikka-spotify")
            .enable_all()
            .build()
            .expect("failed to start rikka-spotify runtime")
    })
}

struct Aborting<T>(JoinHandle<T>);

impl<T> Future for Aborting<T> {
    type Output = Result<T, JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.get_mut().0).poll(cx)
    }
}

impl<T> Drop for Aborting<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub(crate) async fn run<T, F>(work: F) -> Result<T>
where
    T: Send + 'static,
    F: Future<Output = Result<T>> + Send + 'static,
{
    Aborting(runtime().spawn(work))
        .await
        .map_err(|error| Error::Unreadable(error.to_string()))?
}

pub(crate) fn wait<T: Send + 'static>(work: impl Future<Output = T> + Send + 'static) -> Result<T> {
    let (answering, answer) = std::sync::mpsc::sync_channel(1);
    runtime().spawn(async move {
        let _ = answering.send(work.await);
    });
    answer
        .recv()
        .map_err(|_| Error::Unreadable("the Spotify worker dropped the request".into()))
}
