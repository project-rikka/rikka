use std::time::Duration;

use crate::event::PlaybackEvent;
use crate::item::Id;

const SHORTEST: Duration = Duration::from_secs(30);
const ENOUGH: Duration = Duration::from_secs(4 * 60);
const SEEK: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, Default)]
pub struct PlayCounter {
    track: Option<Id>,
    length: Option<Duration>,
    played: Duration,
    at: Option<Duration>,
    playing: bool,
    counted: bool,
}

impl PlayCounter {
    #[must_use]
    pub fn new() -> PlayCounter {
        PlayCounter::default()
    }

    pub fn observe(&mut self, event: &PlaybackEvent) -> Option<Id> {
        let track = match event {
            PlaybackEvent::Advanced { to, .. } => to,
            event => event.track()?,
        };
        if matches!(
            event,
            PlaybackEvent::Loading { .. } | PlaybackEvent::Advanced { .. }
        ) || self.track.as_ref() != Some(track)
        {
            *self = PlayCounter {
                track: Some(track.clone()),
                ..PlayCounter::default()
            };
        }
        match event {
            PlaybackEvent::Length { length, .. } => self.length = Some(*length),
            PlaybackEvent::Playing { at, .. } => {
                self.playing = true;
                self.at = Some(*at);
            }
            PlaybackEvent::Advanced { .. } => {
                self.playing = true;
                self.at = Some(Duration::ZERO);
            }
            PlaybackEvent::Paused { at, .. } => {
                self.advance(*at);
                self.playing = false;
            }
            PlaybackEvent::Position { at, .. } => self.advance(*at),
            PlaybackEvent::Ended { .. } => self.playing = false,
            _ => {}
        }
        if self.counted || !self.enough() {
            return None;
        }
        self.counted = true;
        self.track.clone()
    }

    fn advance(&mut self, to: Duration) {
        if self.playing
            && let Some(from) = self.at
            && to > from
            && to - from <= SEEK
        {
            self.played += to - from;
        }
        self.at = Some(to);
    }

    fn enough(&self) -> bool {
        match self.length {
            Some(length) if length < SHORTEST => false,
            Some(length) => self.played >= (length / 2).min(ENOUGH),
            None => self.played >= ENOUGH,
        }
    }
}
