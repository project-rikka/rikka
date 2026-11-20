use std::time::Duration;

use music_core::Isrc;
use rusqlite::{Params, Transaction, params};

use crate::artwork::ArtworkCache;
use crate::error::Result;
use crate::ids::{AlbumId, ArtistId, TrackId};
use crate::model::Track;
use crate::query::{SongSort, Songs};
use crate::read::rows::{self, TRACKS};
use music_core::fold;

pub(crate) const IN_ALBUM_ORDER: &str =
    "coalesce(t.disc_number, 1), t.track_number IS NULL, t.track_number, t.file_name";

pub(crate) fn select(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    tail: &str,
    values: impl Params,
) -> Result<Vec<Track>> {
    let sql = format!("{TRACKS} {tail}");
    let mut statement = transaction.prepare_cached(&sql)?;
    let tracks = statement
        .query_map(values, |row| rows::track(row, artwork))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(tracks)
}

pub(crate) fn songs(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    songs: Songs,
) -> Result<Vec<Track>> {
    let key = match songs.sort {
        SongSort::Title => "coalesce(t.sort_title, t.match_title)",
        SongSort::Artist => "coalesce(t.sort_artist, t.artist) COLLATE NOCASE",
        SongSort::Album => "coalesce(a.sort_title, t.album) COLLATE NOCASE",
        SongSort::Year => "t.year",
        SongSort::Duration => "t.duration_ms",
        SongSort::Added => "i.added_at",
        SongSort::Plays => "coalesce(l.plays, 0)",
        SongSort::Codec => "t.codec",
    };
    let order = songs.order.sql();
    select(
        transaction,
        artwork,
        &format!("ORDER BY {key} {order}, t.album_id, {IN_ALBUM_ORDER}"),
        [],
    )
}

pub(crate) fn one(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    id: TrackId,
) -> Result<Option<Track>> {
    Ok(select(transaction, artwork, "WHERE t.id = ?1", [id.get()])?
        .into_iter()
        .next())
}

pub(crate) fn by_artist(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    id: ArtistId,
) -> Result<Vec<Track>> {
    select(
        transaction,
        artwork,
        &format!(
            "WHERE a.id IS NOT NULL
               AND (t.id IN (SELECT track FROM track_artists WHERE artist = ?1)
                    OR a.id IN (SELECT album FROM album_artists WHERE artist = ?1))
             ORDER BY a.id NOT IN (SELECT album FROM album_artists WHERE artist = ?1),
                      a.year IS NULL, a.year, a.sort_title, a.id, {IN_ALBUM_ORDER}"
        ),
        [id.get()],
    )
}

pub(crate) fn of_albums(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    albums: &[AlbumId],
) -> Result<Vec<Track>> {
    let listed: Vec<String> = albums.iter().map(|id| id.get().to_string()).collect();
    select(
        transaction,
        artwork,
        &format!(
            "WHERE t.album_id IN (SELECT value FROM json_each(?1))
             ORDER BY t.album_id, {IN_ALBUM_ORDER}"
        ),
        [format!("[{}]", listed.join(","))],
    )
}

pub(crate) fn favourites(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
) -> Result<Vec<Track>> {
    select(
        transaction,
        artwork,
        "WHERE l.favourite_at IS NOT NULL ORDER BY l.favourite_at DESC",
        [],
    )
}

pub(crate) fn most_played(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    limit: usize,
) -> Result<Vec<Track>> {
    select(
        transaction,
        artwork,
        "WHERE l.plays > 0 ORDER BY l.plays DESC, l.last_played DESC LIMIT ?1",
        [i64::try_from(limit).unwrap_or(i64::MAX)],
    )
}

pub(crate) fn by_isrc(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    isrc: &Isrc,
) -> Result<Vec<Track>> {
    select(transaction, artwork, "WHERE t.isrc = ?1", [isrc.as_str()])
}

pub(crate) fn matching(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    title: &str,
    artist: &str,
    duration: Duration,
    window: Duration,
) -> Result<Vec<Track>> {
    let middle = i64::try_from(duration.as_millis()).unwrap_or(i64::MAX);
    let slack = i64::try_from(window.as_millis()).unwrap_or(i64::MAX);
    let artist_key = fold(artist);
    let rows = select(
        transaction,
        artwork,
        "WHERE t.match_title = ?1 AND t.duration_ms BETWEEN ?2 AND ?3",
        params![
            fold(title),
            middle.saturating_sub(slack),
            middle.saturating_add(slack)
        ],
    )?;
    Ok(rows
        .into_iter()
        .filter(|track| {
            artist_key.is_empty()
                || fold(&track.artist).contains(&artist_key)
                || track
                    .artists
                    .iter()
                    .any(|credit| fold(&credit.name) == artist_key)
        })
        .collect())
}
