mod preview;
mod run;

use std::convert::Infallible;

use discord_client::{Broadcast, BroadcastLine};
use discord_core::StreamKey;
use discord_voice::Soundshare;
use futures_channel::mpsc;
use rikka_audio::AudioCapture;
use rikka_screencast::Cast;
use rikka_worker::{CommandSender, Worker, command_channel};

use crate::error::{Error, Result};
use crate::picture::PictureSink;
use crate::quality::Quality;
use crate::update::{Outbox, StreamUpdates};

const THREAD: &str = "rikka-discord-stream-send";

pub(crate) enum Command {
    Pause(bool),
    Quality(Quality),
    Stop,
}

pub struct Broadcaster {
    key: StreamKey,
    line: BroadcastLine,
    updates: Option<StreamUpdates>,
    sending: Option<Worker<CommandSender<Command>>>,
    soundshare: Option<Soundshare>,
}

impl Broadcaster {
    pub fn start(
        broadcast: Broadcast,
        cast: Cast,
        sound: Option<AudioCapture>,
        quality: Quality,
        thumbnail: impl Fn(u32, u32, Vec<u8>) + Send + 'static,
    ) -> Result<Broadcaster> {
        let Broadcast { key, line, events } = broadcast;
        let soundshare = sound
            .map(|sound| Soundshare::start(sound, line.clone()))
            .transpose()?;
        let (commands, received) =
            command_channel(|| Command::Stop).map_err(|source| Error::Thread {
                name: THREAD,
                source,
            })?;
        let (outbox, updates) = Outbox::new();
        let sending_line = line.clone();
        let thumbnail: PictureSink = Box::new(thumbnail);
        let sending = Worker::spawn(THREAD, commands, move || {
            Ok::<_, Infallible>(move || {
                run::run(
                    events,
                    &cast,
                    &sending_line,
                    &received,
                    &outbox,
                    &thumbnail,
                    quality,
                );
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
        Ok(Broadcaster {
            key,
            line,
            updates: Some(updates),
            sending: Some(sending),
            soundshare,
        })
    }

    #[must_use]
    pub fn key(&self) -> StreamKey {
        self.key
    }

    pub fn updates(&mut self) -> StreamUpdates {
        self.updates.take().unwrap_or_else(|| mpsc::unbounded().1)
    }

    pub fn pause(&self, paused: bool) {
        self.tell(Command::Pause(paused));
    }

    pub fn set_quality(&self, quality: Quality) {
        self.tell(Command::Quality(quality));
    }

    pub fn end(mut self) {
        self.shut_down();
    }

    fn tell(&self, command: Command) {
        if let Some(sending) = &self.sending
            && let Err(error) = sending.handle().send(command)
        {
            tracing::debug!(%error, "the stream sender is gone");
        }
    }

    fn shut_down(&mut self) {
        self.sending.take();
        self.soundshare.take();
        self.line.end();
    }
}

impl Drop for Broadcaster {
    fn drop(&mut self) {
        self.shut_down();
    }
}

impl std::fmt::Debug for Broadcaster {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Broadcaster")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}
