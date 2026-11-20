mod attached;
mod outbox;
mod receive;
mod send;
mod shared;
mod update;

use std::sync::Arc;

use discord_client::{Call, Line};
use discord_core::{ChannelId, UserId};
use futures_channel::mpsc;
use rikka_worker::Worker;

use crate::error::{Error, Result};
use crate::io::{Devices, System};
use crate::krisp::Krisp;
use crate::settings::VoiceSettings;
use crate::worker::spawn;

use outbox::Outbox;
use receive::Receiving;
use send::Sending;
use shared::{Bells, Shared};

pub use attached::StreamSound;
pub use update::{VoiceUpdate, VoiceUpdates};

const SEND_THREAD: &str = "rikka-discord-voice-send";
const RECEIVE_THREAD: &str = "rikka-discord-voice-recv";

pub struct Voice {
    channel: ChannelId,
    line: Arc<Line>,
    shared: Arc<Shared>,
    updates: Option<VoiceUpdates>,
    threads: Vec<Worker<Arc<Shared>>>,
    left: bool,
}

impl Voice {
    pub fn start(
        call: Call,
        settings: VoiceSettings,
        krisp: &Krisp,
        muted: bool,
        deafened: bool,
    ) -> Result<Voice> {
        let line = Arc::new(call.line);
        let devices: Arc<dyn Devices> = Arc::new(System::start()?);
        let (shared, Bells { receive, send }) = Shared::new(settings.clone(), muted, deafened)
            .map_err(|source| Error::Thread {
                name: RECEIVE_THREAD,
                source,
            })?;
        let shared = Arc::new(shared);
        let (outbox, updates) = Outbox::new();
        let (sending, reference) = Sending::new(
            Arc::clone(&line),
            Arc::clone(&shared),
            outbox.clone(),
            Arc::clone(&devices),
            &settings,
            krisp.clone(),
        )?;
        let receiving = Receiving::new(
            call.events,
            Arc::clone(&shared),
            outbox,
            devices,
            reference,
            &settings,
        );
        let mut voice = Voice {
            channel: call.channel,
            line,
            shared,
            updates: Some(updates),
            threads: Vec::with_capacity(2),
            left: false,
        };
        voice
            .threads
            .push(spawn(SEND_THREAD, Arc::clone(&voice.shared), move || {
                sending.run(&send);
            })?);
        voice.threads.push(spawn(
            RECEIVE_THREAD,
            Arc::clone(&voice.shared),
            move || {
                receiving.run(&receive);
            },
        )?);
        Ok(voice)
    }

    #[must_use]
    pub fn channel(&self) -> &ChannelId {
        &self.channel
    }

    pub fn set_muted(&self, muted: bool) {
        self.shared.set_muted(muted);
        self.sync_line();
    }

    pub fn set_deafened(&self, deafened: bool) {
        self.shared.set_deafened(deafened);
        self.sync_line();
    }

    pub fn set_settings(&self, settings: VoiceSettings) {
        self.shared.set_settings(settings);
    }

    pub fn set_user_volume(&self, user: &UserId, volume: f32) {
        self.shared.set_user_volume(user, volume);
    }

    pub fn set_user_muted(&self, user: &UserId, muted: bool) {
        self.shared.set_user_muted(user, muted);
    }

    #[must_use]
    pub fn hear_stream(&self) -> StreamSound {
        let (id, packets) = self.shared.attached().attach();
        StreamSound::new(id, packets, Arc::downgrade(&self.shared))
    }

    pub fn push_to_talk(&self, pressed: bool) {
        self.shared.set_push_to_talk(pressed);
    }

    pub fn updates(&mut self) -> VoiceUpdates {
        self.updates.take().unwrap_or_else(|| mpsc::unbounded().1)
    }

    pub fn leave(mut self) {
        self.shut_down();
    }

    fn sync_line(&self) {
        self.line
            .set_self(self.shared.silenced(), self.shared.deafened());
    }

    fn shut_down(&mut self) {
        if std::mem::replace(&mut self.left, true) {
            return;
        }
        self.threads.clear();
        self.line.leave();
    }
}

impl Drop for Voice {
    fn drop(&mut self) {
        self.shut_down();
    }
}

impl std::fmt::Debug for Voice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Voice")
            .field("channel", &self.channel)
            .field("muted", &self.shared.muted())
            .field("deafened", &self.shared.deafened())
            .finish_non_exhaustive()
    }
}
