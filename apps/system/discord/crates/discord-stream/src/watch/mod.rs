mod run;

use std::convert::Infallible;
use std::sync::Arc;

use discord_client::{Watch, WatchLine};
use discord_core::{StreamKey, VoiceEvents};
use discord_voice::StreamSound;
use futures_channel::mpsc;
use rikka_worker::{CommandSender, Worker, command_channel};

use crate::error::{Error, Result};
use crate::picture::PictureSink;
use crate::update::{Outbox, StreamUpdates};

const THREAD: &str = "rikka-discord-stream-watch";

pub(crate) enum Command {
    Fit(u32, u32),
    Stop,
}

pub struct Watcher {
    key: StreamKey,
    line: WatchLine,
    sound: Arc<StreamSound>,
    updates: Option<StreamUpdates>,
    watching: Option<Worker<CommandSender<Command>>>,
}

impl Watcher {
    pub fn start(
        watch: Watch,
        sound: StreamSound,
        picture: impl Fn(u32, u32, Vec<u8>) + Send + 'static,
    ) -> Result<Watcher> {
        let Watch { key, line, events } = watch;
        let sound = Arc::new(sound);
        let (watching, updates) = launch(
            key,
            events,
            line.clone(),
            Arc::clone(&sound),
            Box::new(picture),
        )?;
        Ok(Watcher {
            key,
            line,
            sound,
            updates: Some(updates),
            watching: Some(watching),
        })
    }

    #[must_use]
    pub fn key(&self) -> StreamKey {
        self.key
    }

    pub fn updates(&mut self) -> StreamUpdates {
        self.updates.take().unwrap_or_else(|| mpsc::unbounded().1)
    }

    pub fn set_volume(&self, volume: f32) {
        self.sound.set_volume(volume);
    }

    pub fn fit(&self, width: u32, height: u32) {
        if let Some(watching) = &self.watching
            && let Err(error) = watching.handle().send(Command::Fit(width, height))
        {
            tracing::debug!(%error, "the stream watcher is gone");
        }
    }

    pub fn leave(mut self) {
        self.shut_down();
    }

    fn shut_down(&mut self) {
        self.watching.take();
        self.line.leave();
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.shut_down();
    }
}

impl std::fmt::Debug for Watcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Watcher")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

fn launch(
    key: StreamKey,
    events: VoiceEvents,
    line: WatchLine,
    sound: Arc<StreamSound>,
    picture: PictureSink,
) -> Result<(Worker<CommandSender<Command>>, StreamUpdates)> {
    let (outbox, updates) = Outbox::new();
    let (commands, received) =
        command_channel(|| Command::Stop).map_err(|source| Error::Thread {
            name: THREAD,
            source,
        })?;
    let watching = Worker::spawn(THREAD, commands, move || {
        Ok::<_, Infallible>(move || {
            run::run(key, events, &line, &sound, &picture, &received, &outbox);
        })
    })
    .map_err(|error| match error {
        rikka_worker::Error::Spawn(source) => Error::Thread {
            name: THREAD,
            source,
        },
        rikka_worker::Error::Startup(never) => match never {},
        rikka_worker::Error::Exited => Error::Exited { name: THREAD },
    })?;
    Ok((watching, updates))
}
