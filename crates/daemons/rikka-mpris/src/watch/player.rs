use std::time::{Duration, Instant};

use crate::track::{Abilities, Repeat, Status, Track};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlayerId(pub(crate) String);

impl PlayerId {
    #[must_use]
    pub fn new(bus_name: impl Into<String>) -> PlayerId {
        PlayerId(bus_name.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub id: PlayerId,
    pub identity: String,
    pub desktop_entry: Option<String>,
    pub status: Status,
    pub track: Option<Track>,
    pub position: Duration,
    pub read_at: Instant,
    pub rate: f64,
    pub volume: Option<f64>,
    pub shuffle: Option<bool>,
    pub repeat: Option<Repeat>,
    pub abilities: Abilities,
    pub raises: bool,
    pub active_at: Option<Instant>,
}

impl Player {
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.status == Status::Playing
    }

    #[must_use]
    pub fn position_at(&self, now: Instant) -> Duration {
        if !self.is_playing() {
            return self.position;
        }
        let elapsed = now.saturating_duration_since(self.read_at).as_secs_f64() * self.rate;
        let position = self.position + Duration::from_secs_f64(elapsed.max(0.0));
        match self.track.as_ref().and_then(|track| track.length) {
            Some(length) => position.min(length),
            None => position,
        }
    }

    #[must_use]
    pub fn position_now(&self) -> Duration {
        self.position_at(Instant::now())
    }

    #[must_use]
    pub fn progress(&self) -> Option<f32> {
        let length = self.track.as_ref()?.length?;
        Some((self.position_now().as_secs_f64() / length.as_secs_f64()).clamp(0.0, 1.0) as f32)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Control {
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
}
