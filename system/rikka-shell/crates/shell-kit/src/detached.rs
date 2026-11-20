use std::future::Future;

pub fn detached(name: &'static str, job: impl FnOnce() + Send + 'static) {
    if let Err(error) = std::thread::Builder::new().name(name.to_owned()).spawn(job) {
        tracing::warn!("failed to start {name}: {error}");
    }
}

pub fn detached_future(name: &'static str, work: impl Future<Output = ()> + Send + 'static) {
    detached(name, move || rikka_worker::block_on(work));
}
