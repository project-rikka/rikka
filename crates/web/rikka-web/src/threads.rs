use std::thread::{self, JoinHandle};

pub(crate) fn join(thread: JoinHandle<()>) {
    if thread.thread().id() == thread::current().id() {
        return;
    }
    let name = thread.thread().name().unwrap_or("rikka-web").to_owned();
    if thread.join().is_err() {
        tracing::error!(thread = %name, "engine thread panicked");
    }
}
