mod incoming;
mod panel;
mod talk;

use std::collections::HashSet;
use std::time::Duration;

use discord_core::{ChannelId, Privacy, UserId};
use discord_voice::Voice;
use rikka_app::prelude::*;

pub(crate) use discord_preferences::hotkey;
pub(crate) use incoming::view as incoming_view;
pub(crate) use panel::view as panel_view;
pub(crate) use talk::talk_area;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Seat {
    pub(crate) key: String,
    pub(crate) channel: ChannelId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Joining,
    Connecting,
    Live,
}

#[derive(State)]
pub(crate) struct Calls {
    seat: Tracked<Option<Seat>>,
    phase: Tracked<Phase>,
    privacy: Tracked<Option<Privacy>>,
    latency: Tracked<Option<Duration>>,
    speaking: Tracked<HashSet<UserId>>,
    #[untracked]
    engine: Option<Voice>,
    #[untracked]
    talking: bool,
}

impl Calls {
    pub(crate) fn new() -> Calls {
        Calls {
            seat: Tracked::new(None),
            phase: Tracked::new(Phase::Joining),
            privacy: Tracked::new(None),
            latency: Tracked::new(None),
            speaking: Tracked::new(HashSet::new()),
            engine: None,
            talking: false,
        }
    }

    pub(crate) fn seat(&self) -> Option<&Seat> {
        self.seat.as_ref()
    }

    pub(crate) fn is_seated_in(&self, key: &str, channel: &ChannelId) -> bool {
        self.seat
            .as_ref()
            .is_some_and(|seat| seat.key == key && seat.channel == *channel)
    }

    pub(crate) fn phase(&self) -> Phase {
        *self.phase
    }

    pub(crate) fn privacy(&self) -> Option<&Privacy> {
        self.privacy.as_ref()
    }

    pub(crate) fn latency(&self) -> Option<Duration> {
        *self.latency
    }

    pub(crate) fn is_speaking(&self, user: &UserId) -> bool {
        self.speaking.contains(user)
    }

    pub(crate) fn engine(&self) -> Option<&Voice> {
        self.engine.as_ref()
    }

    pub(crate) fn take_seat(&mut self, seat: Seat) {
        self.seat.set(Some(seat));
        self.phase.set(Phase::Joining);
        self.privacy.set(None);
        self.latency.set(None);
        self.speaking.set(HashSet::new());
        self.talking = false;
    }

    pub(crate) fn start(&mut self, engine: Voice) {
        self.engine = Some(engine);
        self.phase.set(Phase::Connecting);
    }

    pub(crate) fn fallback(&mut self) -> Option<Voice> {
        self.seat.set(None);
        self.privacy.set(None);
        self.latency.set(None);
        self.speaking.set(HashSet::new());
        self.talking = false;
        self.engine.take()
    }

    pub(crate) fn connected(&mut self, privacy: Privacy) {
        self.phase.set(Phase::Live);
        self.privacy.set(Some(privacy));
    }

    pub(crate) fn secured(&mut self, privacy: Privacy) {
        self.privacy.set(Some(privacy));
    }

    pub(crate) fn measured(&mut self, latency: Duration) {
        let shown = |latency: Option<Duration>| latency.map(|latency| latency.as_millis());
        if shown(*self.latency) == shown(Some(latency)) {
            skip_view();
            return;
        }
        self.latency.set(Some(latency));
    }

    pub(crate) fn moved(&mut self, channel: ChannelId) {
        if let Some(seat) = self.seat.as_mut() {
            seat.channel = channel;
        }
    }

    pub(crate) fn speak(&mut self, user: UserId, on: bool) {
        if self.speaking.contains(&user) == on {
            skip_view();
            return;
        }
        match on {
            true => self.speaking.insert(user),
            false => self.speaking.remove(&user),
        };
    }

    pub(crate) fn talk(&mut self, pressed: bool) {
        skip_view();
        if self.talking == pressed {
            return;
        }
        self.talking = pressed;
        if let Some(engine) = &self.engine {
            engine.push_to_talk(pressed);
        }
    }
}

impl std::fmt::Debug for Calls {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Calls")
            .field("seat", &*self.seat)
            .field("phase", &*self.phase)
            .finish_non_exhaustive()
    }
}
