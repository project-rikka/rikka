use std::fs;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, OptionalExtension as _, params};

use crate::error::{Error, Result};

const FILE: &str = "cache.sqlite";
const LAYOUT: i64 = 1;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const DIRECTORY_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS records (
    tbl TEXT NOT NULL,
    key BLOB NOT NULL,
    sealed BLOB NOT NULL,
    used INTEGER NOT NULL,
    PRIMARY KEY (tbl, key)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS records_by_use ON records (tbl, used);
CREATE TABLE IF NOT EXISTS blobs (
    area TEXT NOT NULL,
    key BLOB NOT NULL,
    sealed BLOB NOT NULL,
    size INTEGER NOT NULL,
    used INTEGER NOT NULL,
    PRIMARY KEY (area, key)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS blobs_by_use ON blobs (area, used);
";

const DROPPED: &str = "
DROP TABLE IF EXISTS records;
DROP TABLE IF EXISTS blobs;
";

const SHAPE: [(&str, &[&str]); 2] = [
    ("records", &["tbl", "key", "sealed", "used"]),
    ("blobs", &["area", "key", "sealed", "size", "used"]),
];

#[derive(Debug, Clone)]
pub(crate) enum Location {
    File(PathBuf),
    Memory(String),
}

impl Location {
    pub(crate) fn folder(dir: &Path) -> Result<Location> {
        let failed = |source: std::io::Error| Error::Folder {
            path: dir.to_path_buf(),
            source,
        };
        fs::create_dir_all(dir).map_err(failed)?;
        fs::set_permissions(dir, fs::Permissions::from_mode(DIRECTORY_MODE)).map_err(failed)?;

        let path = dir.join(FILE);
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(FILE_MODE)
            .open(&path)
            .map_err(failed)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(FILE_MODE)).map_err(failed)?;
        Ok(Location::File(path))
    }

    pub(crate) fn memory() -> Location {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        Location::Memory(format!(
            "file:/rikka-proton-cache-{}-{serial}?vfs=memdb",
            process::id()
        ))
    }

    pub(crate) fn path(&self) -> Option<&Path> {
        match self {
            Location::File(path) => Some(path),
            Location::Memory(_) => None,
        }
    }

    pub(crate) fn writer(&self) -> Result<Connection> {
        let connection = self.connect()?;
        if let Location::File(_) = self {
            let mode: String =
                connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
            if !mode.eq_ignore_ascii_case("wal") {
                return Err(Error::Database(format!(
                    "journal mode {mode} instead of WAL"
                )));
            }
        }
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.pragma_update(None, "secure_delete", true)?;
        prepare(&connection)?;
        Ok(connection)
    }

    pub(crate) fn reader(&self) -> Result<Connection> {
        self.connect()
    }

    fn connect(&self) -> Result<Connection> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let connection = match self {
            Location::File(path) => Connection::open_with_flags(path, flags)?,
            Location::Memory(uri) => {
                Connection::open_with_flags(uri, flags | OpenFlags::SQLITE_OPEN_URI)?
            }
        };
        connection.busy_timeout(BUSY_TIMEOUT)?;
        connection.pragma_update(None, "temp_store", "MEMORY")?;
        Ok(connection)
    }
}

fn prepare(connection: &Connection) -> Result<()> {
    let found: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if found != 0 && (found != LAYOUT || !is_shaped(connection)?) {
        tracing::info!(found, "cache schema changed, resetting");
        connection.execute_batch(DROPPED)?;
    }
    connection.execute_batch(SCHEMA)?;
    connection.pragma_update(None, "user_version", LAYOUT)?;
    Ok(())
}

fn is_shaped(connection: &Connection) -> Result<bool> {
    for (table, expected) in SHAPE {
        let mut statement = connection.prepare("SELECT name FROM pragma_table_info(?1)")?;
        let columns = statement
            .query_map([table], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        if columns != expected {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn record(connection: &Connection, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
    Ok(connection
        .prepare_cached("SELECT sealed FROM records WHERE tbl = ?1 AND key = ?2")?
        .query_row(params![table, key], |row| row.get(0))
        .optional()?)
}

pub(crate) fn records(connection: &Connection, table: &str) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
    let mut statement =
        connection.prepare_cached("SELECT key, sealed FROM records WHERE tbl = ?1")?;
    let rows = statement.query_map([table], |row| Ok((row.get(0)?, row.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub(crate) fn blob(connection: &Connection, area: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
    Ok(connection
        .prepare_cached("SELECT sealed FROM blobs WHERE area = ?1 AND key = ?2")?
        .query_row(params![area, key], |row| row.get(0))
        .optional()?)
}

pub(crate) fn last_use(connection: &Connection) -> Result<u64> {
    let last: i64 = connection.query_row(
        "SELECT MAX((SELECT COALESCE(MAX(used), 0) FROM blobs), \
                    (SELECT COALESCE(MAX(used), 0) FROM records))",
        [],
        |row| row.get(0),
    )?;
    Ok(u64::try_from(last).unwrap_or_default())
}

pub(crate) fn put(
    connection: &Connection,
    table: &str,
    key: &[u8],
    sealed: &[u8],
    used: u64,
) -> Result<()> {
    connection
        .prepare_cached(
            "INSERT OR REPLACE INTO records (tbl, key, sealed, used) VALUES (?1, ?2, ?3, ?4)",
        )?
        .execute(params![table, key, sealed, used as i64])?;
    Ok(())
}

pub(crate) fn touch_record(
    connection: &Connection,
    table: &str,
    key: &[u8],
    used: u64,
) -> Result<()> {
    connection
        .prepare_cached("UPDATE records SET used = ?3 WHERE tbl = ?1 AND key = ?2")?
        .execute(params![table, key, used as i64])?;
    Ok(())
}

pub(crate) fn evict_records(connection: &Connection, table: &str, limit: usize) -> Result<()> {
    connection
        .prepare_cached(
            "DELETE FROM records WHERE tbl = ?1 AND key IN \
             (SELECT key FROM records WHERE tbl = ?1 ORDER BY used DESC LIMIT -1 OFFSET ?2)",
        )?
        .execute(params![table, i64::try_from(limit).unwrap_or(i64::MAX)])?;
    Ok(())
}

pub(crate) fn remove(connection: &Connection, table: &str, key: &[u8]) -> Result<()> {
    connection
        .prepare_cached("DELETE FROM records WHERE tbl = ?1 AND key = ?2")?
        .execute(params![table, key])?;
    Ok(())
}

pub(crate) fn discard(
    connection: &Connection,
    table: &str,
    key: &[u8],
    sealed: &[u8],
) -> Result<()> {
    connection
        .prepare_cached("DELETE FROM records WHERE tbl = ?1 AND key = ?2 AND sealed = ?3")?
        .execute(params![table, key, sealed])?;
    Ok(())
}

pub(crate) fn clear_table(connection: &Connection, table: &str) -> Result<()> {
    connection.execute("DELETE FROM records WHERE tbl = ?1", [table])?;
    Ok(())
}

pub(crate) fn wipe(connection: &Connection) -> Result<()> {
    connection.execute_batch("DELETE FROM records; DELETE FROM blobs;")?;
    Ok(())
}

pub(crate) fn put_blob(
    connection: &Connection,
    area: &str,
    key: &[u8],
    sealed: &[u8],
    used: u64,
) -> Result<()> {
    connection
        .prepare_cached(
            "INSERT OR REPLACE INTO blobs (area, key, sealed, size, used) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?
        .execute(params![area, key, sealed, sealed.len() as i64, used as i64])?;
    Ok(())
}

pub(crate) fn touch_blob(connection: &Connection, area: &str, key: &[u8], used: u64) -> Result<()> {
    connection
        .prepare_cached("UPDATE blobs SET used = ?3 WHERE area = ?1 AND key = ?2")?
        .execute(params![area, key, used as i64])?;
    Ok(())
}

pub(crate) fn remove_blob(connection: &Connection, area: &str, key: &[u8]) -> Result<()> {
    connection
        .prepare_cached("DELETE FROM blobs WHERE area = ?1 AND key = ?2")?
        .execute(params![area, key])?;
    Ok(())
}

pub(crate) fn evict(connection: &Connection, area: &str, limit: u64) -> Result<()> {
    let total_size: i64 = connection
        .prepare_cached("SELECT COALESCE(SUM(size), 0) FROM blobs WHERE area = ?1")?
        .query_row([area], |row| row.get(0))?;
    let mut excess = total_size - i64::try_from(limit).unwrap_or(i64::MAX);
    if excess <= 0 {
        return Ok(());
    }

    let mut doomed: Vec<Vec<u8>> = Vec::new();
    {
        let mut oldest = connection
            .prepare_cached("SELECT key, size FROM blobs WHERE area = ?1 ORDER BY used")?;
        let mut rows = oldest.query([area])?;
        while excess > 0
            && let Some(row) = rows.next()?
        {
            let size: i64 = row.get(1)?;
            doomed.push(row.get(0)?);
            excess -= size;
        }
    }
    for key in doomed {
        remove_blob(connection, area, &key)?;
    }
    Ok(())
}
