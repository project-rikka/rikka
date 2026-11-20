use std::ops::ControlFlow;
use std::thread;

use music_core::{Error, Events, Id, PlayCounter, Result};
use music_library::Library;

use crate::items::track_id;

const THREAD: &str = "rikka-music-plays";

pub(crate) fn count(events: Events, library: Library) -> Result<()> {
    thread::Builder::new()
        .name(THREAD.into())
        .spawn(move || {
            let mut counter = PlayCounter::new();
            music_core::drain(events, |event| {
                if let Some(track) = counter.observe(&event) {
                    record(&library, &track);
                }
                ControlFlow::Continue(())
            });
        })
        .map(drop)
        .map_err(|error| Error::Spawn(format!("{THREAD}: {error}")))
}

fn record(library: &Library, track: &Id) {
    let counted = track_id(track)
        .map_err(|error| error.to_string())
        .and_then(|id| library.played(id).map_err(|error| error.to_string()));
    if let Err(error) = counted {
        tracing::warn!("failed to count play of {track}: {error}");
    }
}
