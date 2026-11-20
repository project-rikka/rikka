use pipewire::channel::{Receiver, Sender};
use rikka_worker::{Stop, Worker};

pub type LoopWorker = Worker<LoopStop>;

pub struct LoopStop(Sender<()>);

impl Stop for LoopStop {
    fn stop(&self) {
        if self.0.send(()).is_err() {
            tracing::debug!("PipeWire loop already stopped");
        }
    }
}

impl std::fmt::Debug for LoopStop {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("LoopStop")
    }
}

pub fn spawn<B, R, E>(name: &'static str, body: B) -> Result<LoopWorker, rikka_worker::Error<E>>
where
    B: FnOnce(Receiver<()>) -> Result<R, E> + Send + 'static,
    R: FnOnce() + 'static,
    E: Send + 'static,
{
    let (stop, stopping) = pipewire::channel::channel();
    Worker::spawn(name, LoopStop(stop), move || body(stopping))
}
