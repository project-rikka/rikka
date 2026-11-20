use rusqlite::{Connection, OptionalExtension, params};

use crate::error::Result;
use crate::ids::ProfileId;
use crate::records::Restored;
use crate::time::TimeRange;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Parked {
    table: &'static str,
    stamp: &'static str,
}

pub(crate) const CLOSED: Parked = Parked {
    table: "closed_tabs",
    stamp: "closed_at",
};

pub(crate) const ARCHIVED: Parked = Parked {
    table: "archived_tabs",
    stamp: "archived_at",
};

impl Parked {
    pub(crate) fn take(self, connection: &Connection, id: i64) -> Result<Option<Restored>> {
        let sql = format!(
            "DELETE FROM {} WHERE id = ?1 RETURNING space_id, url, title, state",
            self.table
        );
        Ok(connection
            .prepare_cached(&sql)?
            .query_row([id], Restored::from_row)
            .optional()?)
    }

    pub(crate) fn clear(
        self,
        connection: &Connection,
        profile: ProfileId,
        range: TimeRange,
    ) -> Result<()> {
        let sql = format!(
            "DELETE FROM {} WHERE space_id IN (SELECT id FROM spaces WHERE profile_id = ?1) \
             AND {stamp} >= ?2 AND {stamp} < ?3",
            self.table,
            stamp = self.stamp
        );
        connection.execute(&sql, params![profile, range.from, range.to])?;
        Ok(())
    }
}
