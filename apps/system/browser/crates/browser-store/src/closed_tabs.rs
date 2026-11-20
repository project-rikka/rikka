use rusqlite::{OptionalExtension, params};

use crate::error::Result;
use crate::ids::{ClosedTabId, SpaceId};
use crate::parked::CLOSED;
use crate::records::{Restored, TabRecord, count};
use crate::store::Store;
use crate::time::Timestamp;

pub const CLOSED_TABS_KEPT: usize = 25;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedTab {
    pub id: ClosedTabId,
    pub space: SpaceId,
    pub url: String,
    pub title: String,
    pub closed_at: Timestamp,
}

impl Store {
    pub fn push_closed_tab(&self, space: SpaceId, tab: TabRecord) {
        let closed_at = Timestamp::now();
        self.write(move |connection, _| {
            connection
                .prepare_cached(
                    "INSERT INTO closed_tabs (space_id, url, title, state, closed_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )?
                .execute(params![
                    space,
                    tab.url,
                    tab.title,
                    tab.state.as_deref(),
                    closed_at
                ])?;
            connection
                .prepare_cached(
                    "DELETE FROM closed_tabs WHERE id NOT IN \
                     (SELECT id FROM closed_tabs ORDER BY closed_at DESC, id DESC LIMIT ?1)",
                )?
                .execute([count(CLOSED_TABS_KEPT)])?;
            Ok(())
        });
    }

    pub fn closed_tabs(&self) -> Result<Vec<ClosedTab>> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT id, space_id, url, title, closed_at FROM closed_tabs \
                 ORDER BY closed_at DESC, id DESC",
            )?;
            let rows = statement.query_map([], |row| {
                Ok(ClosedTab {
                    id: row.get(0)?,
                    space: row.get(1)?,
                    url: row.get(2)?,
                    title: row.get(3)?,
                    closed_at: row.get(4)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    pub fn pop_closed_tab(&self) -> Result<Option<(ClosedTab, Restored)>> {
        self.call(|connection, _| {
            Ok(connection
                .prepare_cached(
                    "DELETE FROM closed_tabs WHERE id = \
                     (SELECT id FROM closed_tabs ORDER BY closed_at DESC, id DESC LIMIT 1) \
                     RETURNING space_id, url, title, state, id, closed_at",
                )?
                .query_row([], |row| {
                    let restored = Restored::from_row(row)?;
                    let closed = ClosedTab {
                        id: row.get(4)?,
                        space: restored.space,
                        url: restored.tab.url.clone(),
                        title: restored.tab.title.clone(),
                        closed_at: row.get(5)?,
                    };
                    Ok((closed, restored))
                })
                .optional()?)
        })
    }

    pub fn take_closed_tab(&self, id: ClosedTabId) -> Result<Option<Restored>> {
        self.call(move |connection, _| CLOSED.take(connection, id.get()))
    }
}
