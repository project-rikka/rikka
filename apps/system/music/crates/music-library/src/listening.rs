use rusqlite::{OptionalExtension, Transaction, params};

use crate::error::{Error, Result};
use crate::ids::TrackId;

pub(crate) fn favour(transaction: &Transaction<'_>, id: TrackId, on: bool, now: i64) -> Result<()> {
    known(transaction, id)?;
    transaction.execute(
        "INSERT INTO listening (track, favourite_at) VALUES (?1, ?2)
         ON CONFLICT (track) DO UPDATE SET favourite_at = excluded.favourite_at",
        params![id.get(), on.then_some(now)],
    )?;
    Ok(())
}

pub(crate) fn played(transaction: &Transaction<'_>, id: TrackId, now: i64) -> Result<()> {
    known(transaction, id)?;
    transaction.execute(
        "INSERT INTO listening (track, plays, last_played) VALUES (?1, 1, ?2)
         ON CONFLICT (track) DO UPDATE SET plays = plays + 1, last_played = excluded.last_played",
        params![id.get(), now],
    )?;
    Ok(())
}

fn known(transaction: &Transaction<'_>, id: TrackId) -> Result<()> {
    transaction
        .query_row("SELECT 1 FROM identities WHERE id = ?1", [id.get()], |_| {
            Ok(())
        })
        .optional()?
        .ok_or(Error::UnknownTrack(id))
}
