use std::path::Path;

use rikka_media_format::Kind;

use crate::details::{Details, SoundDetails};

use super::fault::{Fault, Probed, clean};

pub(super) fn read(path: &Path) -> Probed<Details> {
    if rikka_media_format::kind(path) != Some(Kind::Sound) {
        return Err(Fault::Foreign("sound"));
    }
    let tagged = rikka_audio_tags::read(path).map_err(|error| Fault::Import(error.to_string()))?;
    let tags = tagged.tags;
    let named = |name: &Option<String>| name.as_deref().and_then(clean);
    Ok(Details::Sound(Box::new(SoundDetails {
        seconds: tagged.audio.duration.as_secs_f32(),
        title: named(&tags.title),
        artist: clean(&tags.artists.join(", ")),
        album: named(&tags.album),
        cover: tags.cover().is_some(),
    })))
}
