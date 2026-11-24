use std::path::Path;
use std::sync::Arc;

use rikka_audio_tags::{Picture, Tagged};

use crate::abi::violation;
use crate::api::Image;

const NAME_SEPARATOR: &str = ", ";

#[derive(Debug, Default)]
pub(crate) struct SoundTags {
    title: String,
    artist: String,
    album: String,
    album_artist: String,
    track_number: u32,
    duration: f64,
    cover: Option<Arc<Image>>,
    error_message: String,
}

impl SoundTags {
    fn read(path: &Path, cover_size: u32) -> SoundTags {
        let Tagged { tags, audio } = match rikka_audio_tags::read(path) {
            Ok(tagged) => tagged,
            Err(error) => {
                return SoundTags {
                    error_message: format!(
                        "failed to read the tags of {}: {error}",
                        path.display()
                    ),
                    ..SoundTags::default()
                };
            }
        };
        let cover = (cover_size > 0)
            .then(|| tags.cover().or_else(|| tags.pictures.first()))
            .flatten()
            .and_then(|picture| {
                decoded(picture, cover_size)
                    .inspect_err(|error| {
                        tracing::warn!("failed to decode the cover of {}: {error}", path.display());
                    })
                    .ok()
            });
        SoundTags {
            title: tags.title.unwrap_or_default(),
            artist: tags.artists.join(NAME_SEPARATOR),
            album: tags.album.unwrap_or_default(),
            album_artist: tags.album_artists.join(NAME_SEPARATOR),
            track_number: tags.track.number.unwrap_or(0),
            duration: audio.duration.as_secs_f64(),
            cover,
            error_message: String::new(),
        }
    }
}

fn decoded(picture: &Picture, size: u32) -> rikka_image::Result<Arc<Image>> {
    let image = rikka_image::read(picture.data.as_slice())
        .decode()?
        .thumbnail(size, size)?;
    let (width, height) = image.dimensions();
    Ok(Image::rgba(width, height, Arc::from(image.into_rgba8())))
}

pub(crate) fn sound_tags_new(path: &str, cover_size: u32) -> Arc<SoundTags> {
    if path.is_empty() {
        violation("the path is empty");
    }
    Arc::new(SoundTags::read(Path::new(path), cover_size))
}

pub(crate) fn sound_tags_title(tags: &SoundTags) -> String {
    tags.title.clone()
}

pub(crate) fn sound_tags_artist(tags: &SoundTags) -> String {
    tags.artist.clone()
}

pub(crate) fn sound_tags_album(tags: &SoundTags) -> String {
    tags.album.clone()
}

pub(crate) fn sound_tags_album_artist(tags: &SoundTags) -> String {
    tags.album_artist.clone()
}

pub(crate) fn sound_tags_track_number(tags: &SoundTags) -> u32 {
    tags.track_number
}

pub(crate) fn sound_tags_duration(tags: &SoundTags) -> f64 {
    tags.duration
}

pub(crate) fn sound_tags_has_cover(tags: &SoundTags) -> bool {
    tags.cover.is_some()
}

pub(crate) fn sound_tags_cover(tags: &SoundTags) -> Arc<Image> {
    tags.cover
        .clone()
        .unwrap_or_else(|| violation("the sound has no cover; check has_cover first"))
}

pub(crate) fn sound_tags_error_message(tags: &SoundTags) -> String {
    tags.error_message.clone()
}
