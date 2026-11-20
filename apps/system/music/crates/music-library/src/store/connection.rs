use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use crate::error::{Error, Result};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const WRITER_CACHE_KIB: i64 = 16 * 1024;
const READER_CACHE_KIB: i64 = 4 * 1024;
const JOURNAL_LIMIT_BYTES: i64 = 16 * 1024 * 1024;

fn connect(path: &Path, flags: OpenFlags) -> Result<Connection> {
    let connection = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "temp_store", "MEMORY")?;
    Ok(connection)
}

pub(crate) fn writer(path: &Path) -> Result<Connection> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| Error::Open(error.to_string()))?;
    }
    let connection = connect(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?;
    let mode: String = connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(Error::Open(format!("journal mode {mode} instead of WAL")));
    }
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    connection.pragma_update(None, "journal_size_limit", JOURNAL_LIMIT_BYTES)?;
    connection.pragma_update(None, "cache_size", -WRITER_CACHE_KIB)?;
    Ok(connection)
}

pub(crate) fn reader(path: &Path) -> Result<Connection> {
    let connection = connect(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    connection.pragma_update(None, "cache_size", -READER_CACHE_KIB)?;
    Ok(connection)
}
