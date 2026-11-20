use std::time::{Duration, Instant};

use crate::track::{Abilities, Repeat, Status, Track};

#[derive(Clone, Debug, PartialEq)]
pub struct Playback {
    pub status: Status,
    pub track: Option<Track>,
    pub position: Duration,
    pub rate: f64,
    pub volume: f64,
    pub shuffle: Option<bool>,
    pub repeat: Option<Repeat>,
    pub abilities: Abilities,
}

impl Default for Playback {
    fn default() -> Playback {
        Playback {
            status: Status::Stopped,
            track: None,
            position: Duration::ZERO,
            rate: 1.0,
            volume: 1.0,
            shuffle: None,
            repeat: None,
            abilities: Abilities::none(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    SeekBy { forward: bool, by: Duration },
    SeekTo(Duration),
    Volume(f64),
    Shuffle(bool),
    Repeat(Repeat),
    Raise,
    Quit,
}

#[derive(Debug)]
pub(crate) struct PlayerState {
    pub(crate) playback: Playback,
    pub(crate) since: Instant,
}

impl PlayerState {
    pub(crate) fn new() -> PlayerState {
        PlayerState {
            playback: Playback::default(),
            since: Instant::now(),
        }
    }

    pub(crate) fn position(&self, now: Instant) -> Duration {
        let playback = &self.playback;
        if playback.status != Status::Playing {
            return playback.position;
        }
        let elapsed = now.saturating_duration_since(self.since).as_secs_f64() * playback.rate;
        let position = playback.position + Duration::from_secs_f64(elapsed.max(0.0));
        match playback.track.as_ref().and_then(|track| track.length) {
            Some(length) => position.min(length),
            None => position,
        }
    }

    pub(crate) fn jumped(&self, next: &Playback, now: Instant) -> bool {
        let same = match (&self.playback.track, &next.track) {
            (Some(current), Some(next)) => current.id == next.id,
            _ => false,
        };
        let expected = self.position(now).as_secs_f64();
        same && (next.position.as_secs_f64() - expected).abs() > 1.0
    }
}
