use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use crate::error::{Error, Result};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const WRITER_CACHE_KIB: i64 = 8 * 1024;
const READER_CACHE_KIB: i64 = 2 * 1024;
const JOURNAL_LIMIT_BYTES: i64 = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) enum Location {
    File(PathBuf),
    Memory(String),
}

impl Location {
    pub(crate) fn file(path: &Path) -> Result<Self> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|source| Error::CreateFolder {
                path: parent.to_owned(),
                source: Arc::new(source),
            })?;
        }
        Ok(Self::File(path.to_owned()))
    }

    pub(crate) fn memory() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        Self::Memory(format!(
            "file:/rikka-browser-store-{}-{serial}?vfs=memdb",
            process::id()
        ))
    }

    fn connect(&self, create: bool) -> Result<Connection> {
        let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        if create {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        let connection = match self {
            Self::File(path) => Connection::open_with_flags(path, flags),
            Self::Memory(uri) => Connection::open_with_flags(
                uri,
                flags | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_URI,
            ),
        }?;
        connection.busy_timeout(BUSY_TIMEOUT)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "temp_store", "MEMORY")?;
        Ok(connection)
    }

    pub(crate) fn open_writer(&self) -> Result<Connection> {
        let connection = self.connect(true)?;
        if matches!(self, Self::File(_)) {
            let mode: String =
                connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
            if !mode.eq_ignore_ascii_case("wal") {
                return Err(Error::Open(format!("journal mode {mode} instead of WAL")));
            }
        }
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.pragma_update(None, "journal_size_limit", JOURNAL_LIMIT_BYTES)?;
        connection.pragma_update(None, "cache_size", -WRITER_CACHE_KIB)?;
        Ok(connection)
    }

    pub(crate) fn open_reader(&self) -> Result<Connection> {
        let connection = self.connect(false)?;
        connection.pragma_update(None, "cache_size", -READER_CACHE_KIB)?;
        connection.pragma_update(None, "query_only", true)?;
        Ok(connection)
    }
}
