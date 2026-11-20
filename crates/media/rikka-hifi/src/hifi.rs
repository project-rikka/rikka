use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rikka_hifi_core::ReplayGain;
use rikka_worker::{CommandSender, Subscription, Worker};

use crate::device::Device;
use crate::engine::{self, Command, Engine};
use crate::error::{Error, Result};
use crate::event::Event;
use crate::exclusive::cards;
use crate::identity::Identity;
use crate::output::Output;
use crate::source::{Source, SourceId};

#[derive(Clone)]
pub struct Hifi(Arc<Running>);

struct Running {
    worker: Worker<CommandSender<Command>>,
    sources: AtomicU64,
}

impl Hifi {
    pub fn start(identity: Identity) -> Result<Hifi> {
        let (sender, commands) = rikka_worker::command_channel(|| Command::Quit)
            .map_err(|error| Error::Start(error.to_string()))?;
        let worker = Worker::spawn(engine::NAME, sender, move || {
            let engine = Engine::new(identity, commands)?;
            Ok::<_, std::io::Error>(move || engine.run())
        })
        .map_err(|error| Error::Start(error.to_string()))?;
        Ok(Hifi(Arc::new(Running {
            worker,
            sources: AtomicU64::new(1),
        })))
    }

    pub fn devices() -> Result<Vec<Device>> {
        cards::list(true).map_err(|error| Error::ListDevices(error.to_string()))
    }

    #[must_use]
    pub fn events(&self) -> Subscription<Event> {
        let (subscriber, events) = rikka_worker::subscription();
        self.send(Command::Subscribe(subscriber));
        events
    }

    pub fn set_output(&self, output: Output) {
        self.send(Command::Output(output));
    }

    pub fn play(&self, source: Source) -> SourceId {
        let id = self.next_id();
        self.send(Command::Play(id, source));
        id
    }

    pub fn queue(&self, source: Source) -> SourceId {
        let id = self.next_id();
        self.send(Command::Queue(id, source));
        id
    }

    pub fn unqueue(&self) {
        self.send(Command::Unqueue);
    }

    pub fn pause(&self) {
        self.send(Command::Pause);
    }

    pub fn resume(&self) {
        self.send(Command::Resume);
    }

    pub fn stop(&self) {
        self.send(Command::Stop);
    }

    pub fn seek(&self, to: Duration) {
        self.send(Command::Seek(to));
    }

    pub fn set_volume(&self, level: f32) {
        self.send(Command::Volume(level));
    }

    pub fn set_replay_gain(&self, replay_gain: ReplayGain) {
        self.send(Command::ReplayGain(replay_gain));
    }

    fn next_id(&self) -> SourceId {
        SourceId(self.0.sources.fetch_add(1, Ordering::Relaxed))
    }

    fn send(&self, command: Command) {
        if let Err(error) = self.0.worker.handle().send(command) {
            tracing::debug!("failed to send hifi command: {error}");
        }
    }
}

impl std::fmt::Debug for Hifi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hifi").finish_non_exhaustive()
    }
}
