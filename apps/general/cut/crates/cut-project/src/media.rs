use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::rate::Rate;

const RESEMBLANCE: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MediaId(pub(crate) u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stream {
    Picture,
    Sound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rate: Rate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sound {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Media {
    path: PathBuf,
    duration: Duration,
    picture: Option<Picture>,
    sound: Option<Sound>,
}

impl Media {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, duration: Duration) -> Media {
        Media {
            path: path.into(),
            duration,
            picture: None,
            sound: None,
        }
    }

    #[must_use]
    pub fn with_picture(mut self, picture: Picture) -> Media {
        self.picture = Some(picture);
        self
    }

    #[must_use]
    pub fn with_sound(mut self, sound: Sound) -> Media {
        self.sound = Some(sound);
        self
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        self.duration
    }

    #[must_use]
    pub fn frames(&self, rate: Rate) -> i64 {
        rate.frames_in(self.duration)
    }

    #[must_use]
    pub fn picture(&self) -> Option<Picture> {
        self.picture
    }

    #[must_use]
    pub fn sound(&self) -> Option<Sound> {
        self.sound
    }

    #[must_use]
    pub fn has(&self, stream: Stream) -> bool {
        match stream {
            Stream::Picture => self.picture.is_some(),
            Stream::Sound => self.sound.is_some(),
        }
    }

    #[must_use]
    pub fn resembles(&self, other: &Media) -> bool {
        let sized = |media: &Media| media.picture.map(|picture| (picture.width, picture.height));
        sized(self) == sized(other)
            && self.sound.is_some() == other.sound.is_some()
            && self.duration.abs_diff(other.duration) <= RESEMBLANCE
    }
}
