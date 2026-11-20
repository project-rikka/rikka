use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::{Handle, Runtime};

use crate::error::{Error, Result};

const WORKERS: usize = 2;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub(crate) fn handle() -> Handle {
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(WORKERS)
                .thread_name("rikka-discord")
                .enable_all()
                .build()
                .expect("failed to start rikka-discord runtime")
        })
        .handle()
        .clone()
}

pub(crate) async fn run<T, F>(handle: &Handle, work: F) -> Result<T>
where
    F: Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    handle.spawn(work).await.map_err(|_| Error::Cancelled)?
}
