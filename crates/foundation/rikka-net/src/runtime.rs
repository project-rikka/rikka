use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::{Handle, Runtime};

const WORKER_THREADS: usize = 2;

pub(crate) fn shared() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();

    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(WORKER_THREADS)
            .thread_name("rikka-net")
            .enable_all()
            .build()
            .expect("failed to start rikka-net runtime")
    })
}

pub(crate) fn handle() -> Handle {
    Handle::try_current().unwrap_or_else(|_| shared().handle().clone())
}

pub(crate) fn block_on<F>(future: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    shared().spawn(async move {
        let _ = sender.send(future.await);
    });

    receiver.recv().expect("rikka-net task panicked")
}

pub(crate) fn block_on_local<F: Future>(future: F) -> F::Output {
    shared().block_on(future)
}
