use std::collections::BTreeSet;

use music_core::{Error, Result};
use music_library::{AlbumId, AlbumSort, Albums, Library, TrackId};
use rand::Rng;
use rand::seq::SliceRandom;

use crate::off_thread::failed;

const LENGTH: usize = 50;
const ALBUMS_PER_GENRE: usize = 40;
const NEAR_YEARS: u16 = 5;
const SAME_ARTIST: f64 = 3.0;
const SAME_ALBUM_ARTIST: f64 = 2.0;
const SAME_GENRE: f64 = 2.0;
const NEAR_YEAR: f64 = 1.0;
const AKIN: f64 = 2.0;

pub(crate) fn mix(library: &Library, seed: TrackId) -> Result<Vec<music_library::Track>> {
    let seed = library
        .track(seed)
        .map_err(failed)?
        .ok_or_else(|| Error::Missing(format!("track {seed}")))?;
    let mut random = rand::rng();
    let mut albums = BTreeSet::new();
    for artist in &seed.artists {
        if let Some(detail) = library.artist(artist.id).map_err(failed)? {
            albums.extend(
                detail
                    .albums
                    .iter()
                    .chain(&detail.appearances)
                    .map(|album| album.id),
            );
        }
    }
    let genres = library.genres().map_err(failed)?;
    for genre in genres
        .iter()
        .filter(|genre| seed.genres.contains(&genre.name))
    {
        let mut tagged = library
            .albums(Albums::new(AlbumSort::Added).genre(genre.id))
            .map_err(failed)?;
        tagged.shuffle(&mut random);
        albums.extend(tagged.iter().take(ALBUMS_PER_GENRE).map(|album| album.id));
    }
    let albums: Vec<AlbumId> = albums.into_iter().collect();
    let mut drawn = Vec::new();
    for track in library.tracks_of_albums(&albums).map_err(failed)? {
        if track.id == seed.id || !track.available {
            continue;
        }
        let likeness = likeness(&seed, &track);
        if likeness >= AKIN {
            drawn.push((random.random::<f64>().powf(likeness.recip()), track));
        }
    }
    drawn.sort_by(|(left, _), (right, _)| right.total_cmp(left));
    let mut mixed = vec![seed];
    mixed.extend(drawn.into_iter().take(LENGTH - 1).map(|(_, track)| track));
    Ok(mixed)
}

fn likeness(seed: &music_library::Track, track: &music_library::Track) -> f64 {
    let mut likeness = 0.0;
    if track.artists.iter().any(|artist| {
        seed.artists
            .iter()
            .any(|seed_artist| seed_artist.id == artist.id)
    }) {
        likeness += SAME_ARTIST;
    }
    if !seed.album_artist.is_empty() && seed.album_artist == track.album_artist {
        likeness += SAME_ALBUM_ARTIST;
    }
    if track.genres.iter().any(|genre| seed.genres.contains(genre)) {
        likeness += SAME_GENRE;
    }
    if let (Some(seed), Some(track)) = (seed.year, track.year)
        && seed.abs_diff(track) <= NEAR_YEARS
    {
        likeness += NEAR_YEAR;
    }
    likeness
}
