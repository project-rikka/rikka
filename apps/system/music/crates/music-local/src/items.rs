use std::time::Duration;

use music_core::{Album, AlbumRef, Artist, Artwork, Credit, Error, Id, Picture, Release, Track};
use music_library::{AlbumId, ArtistId, ArtistRef, TrackId};

use crate::local::Local;

const SINGLE_TRACKS: u32 = 3;
const EP_TRACKS: u32 = 6;
const EP_LENGTH: Duration = Duration::from_secs(30 * 60);

#[must_use]
pub fn track(row: &music_library::Track) -> Track {
    Track {
        service: Local::SERVICE,
        id: Id::new(row.id.to_string()),
        name: row.title.clone(),
        artists: credits(&row.artists, &row.artist),
        album: AlbumRef {
            id: (row.album.id.get() != 0).then(|| Id::new(row.album.id.to_string())),
            name: row.album.title.clone(),
        },
        artwork: artwork(row.artwork.as_ref()),
        duration: row.duration,
        explicit: false,
        playable: row.available,
        number: row.track.unwrap_or_default(),
        fidelity: Some(row.fidelity),
        isrc: row.isrc.clone(),
    }
}

#[must_use]
pub fn tracks<'a>(rows: impl IntoIterator<Item = &'a music_library::Track>) -> Vec<Track> {
    rows.into_iter().map(track).collect()
}

#[must_use]
pub fn album(row: &music_library::Album) -> Album {
    Album {
        id: Id::new(row.id.to_string()),
        name: row.title.clone(),
        artists: credits(&row.artists, &row.artist),
        artwork: artwork(row.artwork.as_ref()),
        release: release(row),
        year: row.year.map(i32::from),
        fidelity: Some(row.fidelity),
    }
}

#[must_use]
pub fn artist(row: &music_library::Artist) -> Artist {
    Artist {
        id: Id::new(row.id.to_string()),
        name: row.name.clone(),
        artwork: artwork(row.artwork.as_ref()),
    }
}

pub(crate) fn release(row: &music_library::Album) -> Release {
    match row.compilation {
        true => Release::Compilation,
        false if row.tracks <= SINGLE_TRACKS => Release::Single,
        false if row.tracks <= EP_TRACKS && row.duration < EP_LENGTH => Release::Ep,
        false => Release::Album,
    }
}

pub fn track_id(id: &Id) -> Result<TrackId, Error> {
    number(id).map(TrackId::new)
}

pub fn album_id(id: &Id) -> Result<AlbumId, Error> {
    number(id).map(AlbumId::new)
}

pub fn artist_id(id: &Id) -> Result<ArtistId, Error> {
    number(id).map(ArtistId::new)
}

fn number(id: &Id) -> Result<i64, Error> {
    id.as_str()
        .parse()
        .map_err(|_| Error::Missing(format!("{} in the music library", id.as_str())))
}

fn credits(artists: &[ArtistRef], byline: &str) -> Vec<Credit> {
    match artists.is_empty() {
        true if byline.is_empty() => Vec::new(),
        true => vec![Credit {
            id: None,
            name: byline.to_owned(),
        }],
        false => artists
            .iter()
            .map(|artist| Credit {
                id: Some(Id::new(artist.id.to_string())),
                name: artist.name.clone(),
            })
            .collect(),
    }
}

fn artwork(artwork: Option<&music_library::Artwork>) -> Artwork {
    Artwork::new(
        artwork
            .map(|artwork| artwork.files.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|file| Picture {
                url: rikka_open::path_to_uri(&file.path),
                width: file.width,
            })
            .collect(),
    )
}
