use rusqlite::Transaction;

use crate::error::Result;
use crate::ids::GenreId;
use crate::model::Genre;

pub(crate) fn genres(transaction: &Transaction<'_>) -> Result<Vec<Genre>> {
    let genres = transaction
        .prepare_cached(
            "SELECT g.id, g.name, count(DISTINCT t.album_id), count(DISTINCT t.id)
             FROM genres g
             JOIN track_genres tg ON tg.genre = g.id
             JOIN tracks t ON t.id = tg.track
             GROUP BY g.id
             ORDER BY g.folded",
        )?
        .query_map([], |row| {
            Ok(Genre {
                id: GenreId::new(row.get(0)?),
                name: row.get(1)?,
                albums: row.get(2)?,
                tracks: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(genres)
}
