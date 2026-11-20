use rikka_bus::{Backend, EventSink, StartError, Unavailable};
use rikka_worker::{Stop, Worker};

use crate::command::Command;
use crate::error::{Result, startup};
use crate::peaks::Peaks;
use crate::session;
use crate::snapshot::Snapshot;

pub(crate) struct AudioService {
    worker: Worker<Commands>,
    peaks: Peaks,
}

impl AudioService {
    pub(crate) fn start(report: impl Fn(Snapshot) + Send + 'static) -> Result<AudioService> {
        let (commands, receiver) = pipewire::channel::channel();
        let peaks = Peaks::default();

        let watching = peaks.clone();
        let worker = Worker::spawn("rikka-pipewire", Commands(commands), move || {
            session::open(Box::new(report), receiver, watching)
        })
        .map_err(startup)?;

        Ok(AudioService { worker, peaks })
    }

    pub(crate) fn send(&self, command: Command) {
        let _ = self.worker.handle().0.send(command);
    }

    pub(crate) fn peaks(&self) -> &Peaks {
        &self.peaks
    }
}

impl Backend for AudioService {
    type Request = Command;
    type Event = Snapshot;
    type State = Snapshot;

    const UNAVAILABLE: &'static str = "this desktop has no sound to show";

    fn start(report: EventSink<Snapshot>, _: Unavailable) -> Result<AudioService, StartError> {
        Ok(AudioService::start(move |graph| report.emit(graph))?)
    }

    fn request(&self, request: Command) {
        self.send(request);
    }
}

struct Commands(pipewire::channel::Sender<Command>);

impl Stop for Commands {
    fn stop(&self) {
        let _ = self.0.send(Command::Stop);
    }
}

impl std::fmt::Debug for AudioService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioService").finish_non_exhaustive()
    }
}
