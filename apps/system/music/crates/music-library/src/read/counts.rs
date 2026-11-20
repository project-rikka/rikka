use rusqlite::Transaction;

use crate::error::Result;
use crate::model::Counts;

pub(crate) fn counts(transaction: &Transaction<'_>) -> Result<Counts> {
    Ok(transaction
        .prepare_cached(
            "SELECT (SELECT count(*) FROM tracks), (SELECT count(*) FROM albums),
                    (SELECT count(*) FROM artists)",
        )?
        .query_row([], |row| {
            Ok(Counts {
                songs: row.get(0)?,
                albums: row.get(1)?,
                artists: row.get(2)?,
            })
        })?)
}
