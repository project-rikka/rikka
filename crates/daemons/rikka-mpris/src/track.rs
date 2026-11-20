use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Status {
    Playing,
    Paused,
    #[default]
    Stopped,
}

impl Status {
    pub(crate) fn named(name: &str) -> Status {
        match name {
            "Playing" => Status::Playing,
            "Paused" => Status::Paused,
            _ => Status::Stopped,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Status::Playing => "Playing",
            Status::Paused => "Paused",
            Status::Stopped => "Stopped",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Repeat {
    #[default]
    None,
    Track,
    Playlist,
}

impl Repeat {
    pub(crate) fn named(name: &str) -> Option<Repeat> {
        match name {
            "None" => Some(Repeat::None),
            "Track" => Some(Repeat::Track),
            "Playlist" => Some(Repeat::Playlist),
            _ => None,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Repeat::None => "None",
            Repeat::Track => "Track",
            Repeat::Playlist => "Playlist",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Art {
    File(PathBuf),
    Web(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub album_artists: Vec<String>,
    pub length: Option<Duration>,
    pub art: Option<Art>,
    pub url: Option<String>,
    pub number: Option<u32>,
}

impl Track {
    #[must_use]
    pub fn artist(&self) -> Option<String> {
        (!self.artists.is_empty()).then(|| self.artists.join(", "))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Abilities {
    pub play: bool,
    pub pause: bool,
    pub next: bool,
    pub previous: bool,
    pub seek: bool,
    pub control: bool,
}

impl Default for Abilities {
    fn default() -> Abilities {
        Abilities {
            play: true,
            pause: true,
            next: true,
            previous: true,
            seek: true,
            control: true,
        }
    }
}

impl Abilities {
    #[must_use]
    pub fn none() -> Abilities {
        Abilities {
            play: false,
            pause: false,
            next: false,
            previous: false,
            seek: false,
            control: false,
        }
    }
}
