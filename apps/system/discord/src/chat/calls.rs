use discord_core::{ChannelId, UserId};
use discord_voice::{Voice, VoiceUpdate};
use rikka_app::prelude::*;

use super::Chat;
use crate::voice::{Calls, Seat};

const JOIN: &str = "discord-voice-join";
const UPDATES: &str = "discord-voice-updates";
const DECLINE: &str = "discord-voice-decline";

impl Chat {
    pub(crate) fn calls(&self) -> &Calls {
        &self.calls
    }

    pub(crate) fn join_voice(&mut self, channel: &ChannelId) {
        let Some(key) = self.current_key().map(str::to_owned) else {
            return;
        };
        self.join_voice_on(&key, channel);
    }

    pub(crate) fn join_voice_on(&mut self, key: &str, channel: &ChannelId) {
        if self.calls.is_seated_in(key, channel) {
            return;
        }
        let Some(client) = self.linked(key).map(|linked| linked.client().clone()) else {
            return;
        };
        self.leave_voice();
        let seat = Seat {
            key: key.to_owned(),
            channel: *channel,
        };
        self.calls.take_seat(seat.clone());
        let (muted, deafened) = (self.muted(), self.deafened());
        let settings = self.preferences.voice();
        let krisp = self.krisp.clone();
        let channel_id = *channel;
        spawn_keyed(
            JOIN,
            async move {
                let call = client
                    .join_voice(&channel_id, muted, deafened)
                    .await
                    .map_err(|error| error.to_string())?;
                Voice::start(call, settings, &krisp, muted, deafened)
                    .map_err(|error| error.to_string())
            },
            move |chat: &mut Chat, started: Result<Voice, String>| {
                chat.voice_started(&seat, started)
            },
        );
    }

    pub(crate) fn voice_started(&mut self, seat: &Seat, started: Result<Voice, String>) {
        if self.calls.seat() != Some(seat) {
            if let Ok(engine) = started {
                part(engine);
            }
            return;
        }
        match started {
            Ok(mut engine) => {
                for (user, volume) in self.preferences.user_volumes(&seat.key) {
                    engine.set_user_volume(&user, volume);
                }
                let updates = engine.updates();
                self.calls.start(engine);
                stream_keyed(UPDATES, updates, |chat: &mut Chat, update: VoiceUpdate| {
                    chat.handle_voice_update(update);
                });
            }
            Err(reason) => {
                self.calls.fallback();
                self.error_message.set(Some(reason));
            }
        }
    }

    pub(crate) fn leave_voice(&mut self) {
        cancel(JOIN);
        cancel(UPDATES);
        self.end_screens();
        if let Some(engine) = self.calls.fallback() {
            part(engine);
        }
    }

    pub(crate) fn handle_voice_update(&mut self, update: VoiceUpdate) {
        match update {
            VoiceUpdate::Connected(privacy) => {
                self.calls.connected(privacy);
                self.watch_when_seated();
            }
            VoiceUpdate::Privacy(privacy) => self.calls.secured(privacy),
            VoiceUpdate::Speaking { user, on } => self.calls.speak(user, on),
            VoiceUpdate::SelfSpeaking(on) => {
                let me = self
                    .calls
                    .seat()
                    .and_then(|seat| self.linked(&seat.key))
                    .and_then(|linked| linked.me())
                    .map(|me| me.id);
                match me {
                    Some(me) => self.calls.speak(me, on),
                    None => skip_view(),
                }
            }
            VoiceUpdate::Latency(latency) => self.calls.measured(latency),
            VoiceUpdate::Moved(channel) => self.calls.moved(channel),
            VoiceUpdate::DeviceLost => {
                self.error_message
                    .set(Some(tr!("discord.voice.device-lost").to_string()));
            }
            VoiceUpdate::Ended(error_message) => {
                self.leave_voice();
                if let Some(error_message) = error_message {
                    self.error_message.set(Some(error_message.to_string()));
                }
            }
        }
    }

    pub(crate) fn accept_call(&mut self, key: &str, channel: &ChannelId) {
        self.join_voice_on(key, channel);
    }

    pub(crate) fn decline_call(&mut self, key: &str, channel: &ChannelId) {
        let Some(client) = self.linked(key).map(|linked| linked.client().clone()) else {
            return;
        };
        let declined = *channel;
        spawn_keyed(
            (DECLINE, key.to_owned(), *channel),
            async move { client.decline_call(&declined).await },
            |chat: &mut Chat, result: discord_core::Result<()>| {
                if let Err(error) = result {
                    chat.error_message.set(Some(error.to_string()));
                }
            },
        );
    }

    pub(crate) fn push_to_talk(&mut self, pressed: bool) {
        self.calls.talk(pressed);
    }

    pub(crate) fn user_volume(&self, key: &str, user: &UserId) -> f32 {
        self.preferences.user_volume(key, user)
    }

    pub(crate) fn tune_user_volume(&mut self, key: &str, user: &UserId, volume: f32) {
        skip_view();
        self.tune(key, user, volume);
    }

    pub(crate) fn set_user_volume(&mut self, key: &str, user: &UserId, volume: f32) {
        self.tune(key, user, volume);
        self.preferences.set_user_volume(key, user, volume);
    }

    fn tune(&self, key: &str, user: &UserId, volume: f32) {
        if let Some(engine) = self.engine_of(key) {
            engine.set_user_volume(user, volume);
        }
    }

    pub(crate) fn set_voice_settings(&mut self, settings: discord_voice::VoiceSettings) {
        self.preferences.set_voice(&settings);
        if let Some(engine) = self.calls.engine() {
            engine.set_settings(settings);
        }
    }

    pub(crate) fn sync_engine(&self) {
        if let Some(engine) = self.calls.engine() {
            engine.set_muted(self.muted());
            engine.set_deafened(self.deafened());
        }
    }

    fn engine_of(&self, key: &str) -> Option<&Voice> {
        self.calls
            .seat()
            .filter(|seat| seat.key == key)
            .and_then(|_| self.calls.engine())
    }
}

fn part(engine: Voice) {
    spawn_blocking(move || engine.leave(), |(): ()| skip_view());
}
