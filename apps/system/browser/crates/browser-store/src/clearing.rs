use rusqlite::Connection;

use crate::error::Result;
use crate::favicons;
use crate::ids::ProfileId;
use crate::parked::{ARCHIVED, CLOSED};
use crate::store::Store;
use crate::time::TimeRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DataKinds {
    pub closed_tabs: bool,
    pub archive: bool,
}

impl DataKinds {
    #[must_use]
    pub const fn all() -> Self {
        Self {
            closed_tabs: true,
            archive: true,
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
    if kinds.closed_tabs {
        CLOSED.clear(connection, profile, range)?;
    }
    if kinds.archive {
        ARCHIVED.clear(connection, profile, range)?;
    }
    if kinds.closed_tabs || kinds.archive {
        favicons::collect_garbage(connection)?;
    }
    Ok(())
}
