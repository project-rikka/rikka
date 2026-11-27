use std::sync::atomic::{AtomicI64, Ordering};

use rusqlite::{Connection, OptionalExtension};

use crate::error::Result;

#[derive(Debug)]
pub(crate) struct Sequence(AtomicI64);

impl Sequence {
    pub(crate) fn next(&self) -> i64 {
        self.0.fetch_add(1, Ordering::Relaxed) + 1
    }
}

#[derive(Debug)]
pub(crate) struct Sequences {
    pub(crate) profile: Sequence,
    pub(crate) space: Sequence,
    pub(crate) favorite: Sequence,
    pub(crate) folder: Sequence,
    pub(crate) pinned_tab: Sequence,
    pub(crate) archived_tab: Sequence,
    pub(crate) tab: Sequence,
    pub(crate) split_group: Sequence,
}

impl Sequences {
    pub(crate) fn load(connection: &Connection) -> Result<Self> {
        let issued = |table: &str| -> Result<Sequence> {
            let recorded: Option<i64> = connection
                .query_row(
                    "SELECT seq FROM sqlite_sequence WHERE name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .optional()?;
            let highest = highest(connection, &format!("SELECT max(id) FROM {table}"))?;
            Ok(Sequence(AtomicI64::new(recorded.unwrap_or(0).max(highest))))
        };
        Ok(Self {
            profile: issued("profiles")?,
            space: issued("spaces")?,
            favorite: issued("favorites")?,
            folder: issued("pinned_folders")?,
            pinned_tab: issued("pinned_tabs")?,
            archived_tab: issued("archived_tabs")?,
            tab: Sequence(AtomicI64::new(highest(
                connection,
                "SELECT max(id) FROM session_tabs",
            )?)),
            split_group: Sequence(AtomicI64::new(highest(
                connection,
                "SELECT max(split_group) FROM session_tabs",
            )?)),
        })
    }
}

fn highest(connection: &Connection, sql: &str) -> Result<i64> {
    let value: Option<i64> = connection.query_row(sql, [], |row| row.get(0))?;
    Ok(value.unwrap_or(0))
}
