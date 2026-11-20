use std::time::Duration;

use music_core::{Codec, Fidelity, Isrc};
use music_tags::{Gain, ReplayGain};
use rusqlite::Row;

use crate::artwork::ArtworkCache;
use crate::ids::{AlbumId, ArtistId, TrackId};
use crate::model::{Album, AlbumRef, Artist, ArtistRef, Listening, Track};
use crate::paths;
use crate::time;

const LIST_SEPARATOR: char = '\u{1e}';
const PAIR_SEPARATOR: char = '\u{1f}';

pub(crate) const TRACKS: &str = "
SELECT t.id, t.path, coalesce(r.online, 0), t.title, t.artist, t.album_id,
       coalesce(a.title, t.album, ''), coalesce(t.album_artist, a.artist, t.artist),
       t.track_number, t.disc_number, t.disc_subtitle, t.year, t.composer, t.isrc,
       t.mb_recording, t.duration_ms, t.codec, t.lossless, t.sample_rate, t.bit_depth,
       t.channels, t.bitrate, t.track_gain, t.track_peak, t.album_gain, t.album_peak,
       w.hash, w.extension, w.width,
       l.favourite_at IS NOT NULL, coalesce(l.plays, 0), l.last_played, i.added_at,
       (SELECT group_concat(ta.artist || char(31) || ar.name, char(30) ORDER BY ta.position)
          FROM track_artists ta JOIN artists ar ON ar.id = ta.artist WHERE ta.track = t.id),
       (SELECT group_concat(g.name, char(30) ORDER BY g.folded)
          FROM track_genres tg JOIN genres g ON g.id = tg.genre WHERE tg.track = t.id)
FROM tracks t
LEFT JOIN roots r ON r.path = t.root
LEFT JOIN albums a ON a.id = t.album_id
LEFT JOIN artwork w ON w.hash = coalesce(t.cover, a.artwork)
LEFT JOIN listening l ON l.track = t.id
JOIN identities i ON i.id = t.id
";

pub(crate) const ALBUMS: &str = "
SELECT a.id, a.title, a.artist, a.year, a.compilation, a.tracks, a.discs, a.duration_ms,
       a.codec, a.lossless, a.bit_depth, a.sample_rate, w.hash, w.extension, w.width,
       a.added_at,
       (SELECT group_concat(aa.artist || char(31) || ar.name, char(30) ORDER BY aa.position)
          FROM album_artists aa JOIN artists ar ON ar.id = aa.artist WHERE aa.album = a.id),
       EXISTS (SELECT 1 FROM tracks t JOIN roots r ON r.path = t.root
               WHERE t.album_id = a.id AND r.online)
FROM albums a
LEFT JOIN artwork w ON w.hash = a.artwork
";

pub(crate) const ARTISTS: &str = "
SELECT ar.id, ar.name, ar.albums, ar.tracks, w.hash, w.extension, w.width
FROM artists ar
LEFT JOIN artwork w ON w.hash = ar.artwork
";

pub(crate) fn track(row: &Row<'_>, artwork: &ArtworkCache) -> rusqlite::Result<Track> {
    let fidelity = Fidelity::new(
        codec(row.get(16)?),
        row.get(17)?,
        row.get(19)?,
        row.get(18)?,
        row.get(21)?,
    );
    let album_id: Option<i64> = row.get(5)?;
    let last_played: Option<i64> = row.get(31)?;
    Ok(Track {
        id: TrackId::new(row.get(0)?),
        path: paths::from_bytes(row.get(1)?),
        available: row.get(2)?,
        title: row.get(3)?,
        artist: row.get(4)?,
        album: AlbumRef {
            id: AlbumId::new(album_id.unwrap_or_default()),
            title: row.get(6)?,
        },
        album_artist: row.get(7)?,
        track: row.get(8)?,
        disc: row.get(9)?,
        disc_subtitle: row.get(10)?,
        year: row.get(11)?,
        composer: row.get(12)?,
        isrc: row
            .get::<_, Option<String>>(13)?
            .as_deref()
            .and_then(Isrc::parse),
        musicbrainz_recording: row.get(14)?,
        duration: millis(row.get(15)?),
        fidelity,
        channels: row.get(20)?,
        gain: ReplayGain {
            track: gain(row.get(22)?, row.get(23)?),
            album: gain(row.get(24)?, row.get(25)?),
        },
        artwork: picture(row, 26, artwork)?,
        listening: Listening {
            favourite: row.get(29)?,
            plays: row.get(30)?,
            last_played: last_played.map(time::instant),
        },
        added: time::instant(row.get(32)?),
        artists: pairs(row.get(33)?)
            .map(|(id, name)| ArtistRef {
                id: ArtistId::new(id),
                name,
            })
            .collect(),
        genres: list(row.get(34)?),
    })
}

pub(crate) fn album(row: &Row<'_>, artwork: &ArtworkCache) -> rusqlite::Result<Album> {
    let fidelity = Fidelity::new(
        codec(row.get(8)?),
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        None,
    );
    Ok(Album {
        id: AlbumId::new(row.get(0)?),
        title: row.get(1)?,
        artist: row.get(2)?,
        year: row.get(3)?,
        compilation: row.get(4)?,
        tracks: row.get(5)?,
        discs: row.get(6)?,
        duration: millis(row.get(7)?),
        fidelity,
        artwork: picture(row, 12, artwork)?,
        added: time::instant(row.get(15)?),
        artists: pairs(row.get(16)?)
            .map(|(id, name)| ArtistRef {
                id: ArtistId::new(id),
                name,
            })
            .collect(),
        available: row.get(17)?,
    })
}

pub(crate) fn artist(row: &Row<'_>, artwork: &ArtworkCache) -> rusqlite::Result<Artist> {
    Ok(Artist {
        id: ArtistId::new(row.get(0)?),
        name: row.get(1)?,
        albums: row.get(2)?,
        tracks: row.get(3)?,
        artwork: picture(row, 4, artwork)?,
    })
}

fn picture(
    row: &Row<'_>,
    at: usize,
    artwork: &ArtworkCache,
) -> rusqlite::Result<Option<crate::model::Artwork>> {
    let hash: Option<String> = row.get(at)?;
    let extension: Option<String> = row.get(at + 1)?;
    let width: Option<u32> = row.get(at + 2)?;
    Ok(hash
        .zip(extension)
        .zip(width)
        .map(|((hash, extension), width)| artwork.artwork(&hash, &extension, width)))
}

fn codec(slug: String) -> Codec {
    Codec::from_slug(&slug)
}

fn millis(value: i64) -> Duration {
    Duration::from_millis(u64::try_from(value).unwrap_or(0))
}

fn gain(db: Option<f32>, peak: Option<f32>) -> Option<Gain> {
    db.map(|db| Gain { db, peak })
}

fn list(joined: Option<String>) -> Vec<String> {
    joined
        .map(|joined| joined.split(LIST_SEPARATOR).map(str::to_owned).collect())
        .unwrap_or_default()
}

fn pairs(joined: Option<String>) -> impl Iterator<Item = (i64, String)> {
    list(joined).into_iter().filter_map(|pair| {
        let (id, name) = pair.split_once(PAIR_SEPARATOR)?;
        Some((id.parse().ok()?, name.to_owned()))
    })
}
