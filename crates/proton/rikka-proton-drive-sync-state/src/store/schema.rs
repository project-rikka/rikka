use rusqlite::Connection;

use crate::error::{Error, Result};

const VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

CREATE TABLE roots (
    id INTEGER PRIMARY KEY,
    local_path BLOB NOT NULL UNIQUE,
    target TEXT NOT NULL,
    remote_path TEXT NOT NULL,
    volume TEXT NOT NULL,
    share TEXT NOT NULL,
    remote_root TEXT NOT NULL,
    local_root INTEGER NOT NULL
) STRICT;

CREATE TABLE cursors (
    volume TEXT PRIMARY KEY,
    event TEXT NOT NULL
) STRICT;

CREATE TABLE local_items (
    root INTEGER NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
    inode INTEGER NOT NULL,
    parent INTEGER NOT NULL,
    name TEXT NOT NULL,
    kind INTEGER NOT NULL,
    size INTEGER NOT NULL,
    modified INTEGER NOT NULL,
    sha1 TEXT,
    PRIMARY KEY (root, inode)
) STRICT, WITHOUT ROWID;

CREATE TABLE remote_items (
    root INTEGER NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
    link TEXT NOT NULL,
    parent TEXT NOT NULL,
    name TEXT NOT NULL,
    kind INTEGER NOT NULL,
    revision TEXT,
    sha1 TEXT,
    size INTEGER NOT NULL,
    modified INTEGER NOT NULL,
    PRIMARY KEY (root, link)
) STRICT, WITHOUT ROWID;

CREATE TABLE synced (
    root INTEGER NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
    id INTEGER NOT NULL,
    kind INTEGER NOT NULL,
    local INTEGER NOT NULL,
    local_parent INTEGER NOT NULL,
    local_name TEXT NOT NULL,
    remote TEXT NOT NULL,
    remote_parent TEXT NOT NULL,
    remote_name TEXT NOT NULL,
    sha1 TEXT,
    size INTEGER NOT NULL,
    revision TEXT,
    PRIMARY KEY (root, id)
) STRICT, WITHOUT ROWID;

CREATE TABLE journal (
    id INTEGER PRIMARY KEY,
    root INTEGER NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
    operation TEXT NOT NULL,
    running INTEGER NOT NULL DEFAULT 0,
    artifact BLOB,
    attempts INTEGER NOT NULL DEFAULT 0,
    retry_at INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    UNIQUE (root, operation)
) STRICT;

CREATE TABLE drafts (
    entry INTEGER PRIMARY KEY REFERENCES journal(id) ON DELETE CASCADE,
    link TEXT NOT NULL,
    revision TEXT NOT NULL
) STRICT;

CREATE TABLE draft_blocks (
    entry INTEGER NOT NULL REFERENCES drafts(entry) ON DELETE CASCADE,
    block INTEGER NOT NULL,
    hash BLOB NOT NULL,
    size INTEGER NOT NULL,
    digest BLOB NOT NULL,
    PRIMARY KEY (entry, block)
) STRICT, WITHOUT ROWID;
";

pub(super) fn prepare(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction()?;
    let found: i64 = transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    match found {
        VERSION => return Ok(()),
        0 => {
            transaction.execute_batch(SCHEMA)?;
            transaction.pragma_update(None, "user_version", VERSION)?;
        }
        _ => return Err(Error::UnknownSchema { found }),
    }
    transaction.commit()?;
    Ok(())
}
