use std::convert::Infallible;

use rikka_worker::{Stop, Worker};

use crate::error::{Error, Result};

pub(crate) fn spawn<H: Stop>(
    name: &'static str,
    handle: H,
    body: impl FnOnce() + Send + 'static,
) -> Result<Worker<H>> {
    Worker::spawn(name, handle, move || Ok::<_, Infallible>(body)).map_err(|error| match error {
        rikka_worker::Error::Spawn(source) => Error::Thread { name, source },
        rikka_worker::Error::Startup(never) => match never {},
        rikka_worker::Error::Exited => Error::Exited { name },
    })
}
