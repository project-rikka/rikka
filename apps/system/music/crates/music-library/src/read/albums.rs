use rusqlite::{Params, Transaction, params};

use crate::artwork::ArtworkCache;
use crate::error::Result;
use crate::ids::AlbumId;
use crate::model::{Album, AlbumDetail};
use crate::query::{AlbumSort, Albums};
use crate::read::rows::{self, ALBUMS};
use crate::read::tracks::{self, IN_ALBUM_ORDER};

pub(crate) fn select(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    tail: &str,
    values: impl Params,
) -> Result<Vec<Album>> {
    let sql = format!("{ALBUMS} {tail}");
    let albums = transaction
        .prepare_cached(&sql)?
        .query_map(values, |row| rows::album(row, artwork))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(albums)
}

pub(crate) fn albums(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    query: Albums,
) -> Result<Vec<Album>> {
    let key = match query.sort {
        AlbumSort::Artist => "a.sort_artist {order}, a.year, a.sort_title",
        AlbumSort::Title => "a.sort_title {order}, a.sort_artist",
        AlbumSort::Year => "a.year IS NULL, a.year {order}, a.sort_artist, a.sort_title",
        AlbumSort::Added => "a.added_at {order}, a.sort_artist, a.sort_title",
    }
    .replace("{order}", query.order.sql());
    select(
        transaction,
        artwork,
        &format!(
            "WHERE (?1 IS NULL OR a.id IN (
                SELECT t.album_id FROM tracks t JOIN track_genres g ON g.track = t.id
                WHERE g.genre = ?1))
             AND (?2 IS NULL OR a.id IN (SELECT album FROM album_artists WHERE artist = ?2)
                  OR a.id IN (SELECT t.album_id FROM tracks t
                              JOIN track_artists ta ON ta.track = t.id WHERE ta.artist = ?2))
             ORDER BY {key}"
        ),
        params![
            query.genre.map(|genre| genre.get()),
            query.artist.map(|artist| artist.get())
        ],
    )
}

pub(crate) fn recently_added(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    limit: usize,
) -> Result<Vec<Album>> {
    select(
        transaction,
        artwork,
        "ORDER BY a.added_at DESC, a.sort_artist LIMIT ?1",
        [i64::try_from(limit).unwrap_or(i64::MAX)],
    )
}

pub(crate) fn detail(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    id: AlbumId,
) -> Result<Option<AlbumDetail>> {
    let Some(album) = select(transaction, artwork, "WHERE a.id = ?1", [id.get()])?
        .into_iter()
        .next()
    else {
        return Ok(None);
    };
    let tracks = tracks::select(
        transaction,
        artwork,
        &format!("WHERE t.album_id = ?1 ORDER BY {IN_ALBUM_ORDER}"),
        [id.get()],
    )?;
    Ok(Some(AlbumDetail { album, tracks }))
}
