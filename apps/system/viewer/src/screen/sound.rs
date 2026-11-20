use std::path::{Path, PathBuf};

use rikka_app::prelude::*;
use rikka_audio::{PlayParams, Track};

use crate::screen::Screen;

pub(crate) type Opened = rikka_audio::Result<Track>;

#[derive(Debug)]
pub(crate) enum Sound {
    Opening,
    Ready(Box<Track>),
    Unplayable(rikka_audio::Error),
}

impl Sound {
    pub(crate) fn track(&self) -> Option<&Track> {
        match self {
            Self::Ready(track) => Some(track.as_ref()),
            Self::Opening | Self::Unplayable(_) => None,
        }
    }

    pub(crate) fn track_mut(&mut self) -> Option<&mut Track> {
        match self {
            Self::Ready(track) => Some(track.as_mut()),
            Self::Opening | Self::Unplayable(_) => None,
        }
    }
}

impl From<Opened> for Sound {
    fn from(opened: Opened) -> Self {
        match opened {
            Ok(track) => Self::Ready(Box::new(track)),
            Err(error) => Self::Unplayable(error),
        }
    }
}

pub(crate) fn stream(path: &Path, on_opened: fn(&mut Screen, &Path, Opened)) {
    let path = path.to_path_buf();
    spawn_blocking(
        move || {
            let track = Track::stream(&path, PlayParams::new());
            (path, track)
        },
        move |screen: &mut Screen, (path, opened): (PathBuf, Opened)| {
            on_opened(screen, &path, opened);
        },
    );
}
