use rusqlite::Transaction;

use crate::artwork::ArtworkCache;
use crate::error::Result;
use crate::ids::ArtistId;
use crate::model::{Artist, ArtistDetail};
use crate::read::albums;
use crate::read::rows::{self, ARTISTS};

pub(crate) fn artists(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
) -> Result<Vec<Artist>> {
    let artists = transaction
        .prepare_cached(&format!("{ARTISTS} ORDER BY ar.sort_name"))?
        .query_map([], |row| rows::artist(row, artwork))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(artists)
}

pub(crate) fn artist(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    id: ArtistId,
) -> Result<Option<ArtistDetail>> {
    let stored: Option<Artist> = transaction
        .prepare_cached(&format!("{ARTISTS} WHERE ar.id = ?1"))?
        .query_map([id.get()], |row| rows::artist(row, artwork))?
        .next()
        .transpose()?;
    let Some(artist) = stored else {
        return Ok(None);
    };
    let albums = albums::select(
        transaction,
        artwork,
        "WHERE a.id IN (SELECT album FROM album_artists WHERE artist = ?1)
         ORDER BY a.year IS NULL, a.year, a.sort_title",
        [id.get()],
    )?;
    let appearances = albums::select(
        transaction,
        artwork,
        "WHERE a.id NOT IN (SELECT album FROM album_artists WHERE artist = ?1)
           AND a.id IN (SELECT t.album_id FROM tracks t
                        JOIN track_artists ta ON ta.track = t.id WHERE ta.artist = ?1)
         ORDER BY a.year IS NULL, a.year, a.sort_title",
        [id.get()],
    )?;
    Ok(Some(ArtistDetail {
        artist,
        albums,
        appearances,
    }))
}
