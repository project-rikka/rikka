use rusqlite::{Connection, params};

use crate::downloads::DownloadState;
use crate::error::Result;
use crate::favicons;
use crate::history;
use crate::ids::ProfileId;
use crate::parked::{ARCHIVED, CLOSED};
use crate::store::Store;
use crate::time::TimeRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DataKinds {
    pub history: bool,
    pub downloads: bool,
    pub closed_tabs: bool,
    pub archive: bool,
    pub site_settings: bool,
}

impl DataKinds {
    #[must_use]
    pub const fn all() -> Self {
        Self {
            history: true,
            downloads: true,
            closed_tabs: true,
            archive: true,
            site_settings: true,
        }
    }
}

impl Store {
    pub fn clear_browsing_data(&self, profile: ProfileId, kinds: DataKinds, range: TimeRange) {
        self.write(move |connection, _| clear(connection, profile, kinds, range));
    }
}

fn clear(
    connection: &Connection,
    profile: ProfileId,
    kinds: DataKinds,
    range: TimeRange,
) -> Result<()> {
    let window = |sql: &str| -> Result<()> {
        connection.execute(sql, params![profile, range.from, range.to])?;
        Ok(())
    };
    if kinds.history {
        history::delete_range(connection, profile, range)?;
    }
    if kinds.downloads {
        connection.execute(
            "DELETE FROM downloads WHERE profile_id = ?1 \
             AND started_at >= ?2 AND started_at < ?3 AND state != ?4",
            params![profile, range.from, range.to, DownloadState::IN_PROGRESS],
        )?;
    }
    if kinds.closed_tabs {
        CLOSED.clear(connection, profile, range)?;
    }
    if kinds.archive {
        ARCHIVED.clear(connection, profile, range)?;
    }
    if kinds.history || kinds.closed_tabs || kinds.archive {
        favicons::collect_garbage(connection)?;
    }
    if kinds.site_settings {
        window(
            "DELETE FROM site_permissions WHERE profile_id = ?1 \
             AND changed_at >= ?2 AND changed_at < ?3",
        )?;
        window(
            "DELETE FROM site_hosts WHERE profile_id = ?1 \
             AND changed_at >= ?2 AND changed_at < ?3",
        )?;
    }
    Ok(())
}
