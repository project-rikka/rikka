use std::time::Duration;

use rikka_worker::Subscription;

use crate::fidelity::Fidelity;
use crate::item::Id;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaybackEvent {
    Loading { track: Id },
    Playing { track: Id, at: Duration },
    Paused { track: Id, at: Duration },
    Position { track: Id, at: Duration },
    Length { track: Id, length: Duration },
    Sounding { track: Id, fidelity: Fidelity },
    Ended { track: Id },
    Advanced { from: Id, to: Id },
    Unavailable { track: Id },
    Refused { track: Id },
    Failed(String),
}

impl PlaybackEvent {
    #[must_use]
    pub fn track(&self) -> Option<&Id> {
        match self {
            PlaybackEvent::Loading { track }
            | PlaybackEvent::Playing { track, .. }
            | PlaybackEvent::Paused { track, .. }
            | PlaybackEvent::Position { track, .. }
            | PlaybackEvent::Length { track, .. }
            | PlaybackEvent::Sounding { track, .. }
            | PlaybackEvent::Ended { track }
            | PlaybackEvent::Advanced { from: track, .. }
            | PlaybackEvent::Unavailable { track }
            | PlaybackEvent::Refused { track } => Some(track),
            PlaybackEvent::Failed(_) => None,
        }
    }
}

pub type Events = Subscription<PlaybackEvent>;
