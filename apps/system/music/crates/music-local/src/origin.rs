use music_core::Id;
use music_library::Library;
use music_stream::{Located, Origin};
use music_tags::{Gain, ReplayGain};
use rikka_hifi::{Gains, Loudness};

use crate::items::track_id;

pub(crate) struct LocalOrigin {
    library: Library,
}

impl LocalOrigin {
    pub(crate) fn new(library: Library) -> LocalOrigin {
        LocalOrigin { library }
    }
}

impl Origin for LocalOrigin {
    fn locate(&self, track: &Id) -> Option<Located> {
        let stored = self
            .library
            .track(track_id(track).ok()?)
            .inspect_err(|error| tracing::warn!("failed to look up {track}: {error}"))
            .ok()??;
        Some(Located {
            path: stored.path,
            gains: gains(&stored.gain),
        })
    }
}

fn gains(gain: &ReplayGain) -> Gains {
    Gains {
        track: gain.track.map(loudness),
        album: gain.album.map(loudness),
    }
}

fn loudness(gain: Gain) -> Loudness {
    Loudness {
        gain_db: gain.db,
        peak: gain.peak,
    }
}
