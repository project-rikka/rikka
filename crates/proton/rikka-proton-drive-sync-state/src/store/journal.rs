use std::collections::HashMap;
use std::fmt;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

use jiff::Timestamp;
use rusqlite::{Row, params};

use super::{Store, optional_path, write_changes};
use crate::error::Result;
use crate::plan::Operation;
use crate::root::RootId;
use crate::state::Changes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId(pub(crate) i64);

impl fmt::Display for EntryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: EntryId,
    pub root: RootId,
    pub operation: Operation,
    pub running: bool,
    pub attempts: u32,
    pub retry_at: Option<Timestamp>,
    pub error: Option<String>,
    pub artifact: Option<PathBuf>,
}

impl Entry {
    #[must_use]
    pub fn is_due(&self, now: Timestamp) -> bool {
        self.retry_at.is_none_or(|at| at <= now)
    }
}

const COLUMNS: &str = "id, root, operation, running, attempts, retry_at, error, artifact";

impl Store {
    pub fn schedule(&mut self, root: RootId, operations: &[Operation]) -> Result<Vec<Entry>> {
        let transaction = self.connection.transaction()?;
        let mut pending_by_operation: HashMap<String, Entry> = HashMap::new();
        let mut unreadable = Vec::new();
        {
            let mut statement = transaction.prepare_cached(&format!(
                "SELECT {COLUMNS} FROM journal WHERE root = ?1 AND running = 0"
            ))?;
            let rows = statement.query_map([root.0], |row| {
                let id: i64 = row.get(0)?;
                let text: String = row.get(2)?;
                Ok((id, text, read_entry(row)?))
            })?;
            for row in rows {
                match row? {
                    (_, text, Ok(entry)) => {
                        pending_by_operation.insert(text, entry);
                    }
                    (id, _, Err(_)) => unreadable.push(id),
                }
            }
        }

        let mut scheduled = Vec::with_capacity(operations.len());
        {
            let mut insert = transaction.prepare_cached(
                "INSERT INTO journal (root, operation) VALUES (?1, ?2)
                 ON CONFLICT (root, operation) DO NOTHING",
            )?;
            for operation in operations {
                let text = serde_json::to_string(operation)?;
                match pending_by_operation.remove(&text) {
                    Some(entry) => scheduled.push(entry),
                    None => {
                        if insert.execute(params![root.0, text])? == 0 {
                            continue;
                        }
                        scheduled.push(Entry {
                            id: EntryId(transaction.last_insert_rowid()),
                            root,
                            operation: operation.clone(),
                            running: false,
                            attempts: 0,
                            retry_at: None,
                            error: None,
                            artifact: None,
                        });
                    }
                }
            }
            let mut delete = transaction.prepare_cached("DELETE FROM journal WHERE id = ?1")?;
            for stale in pending_by_operation.values() {
                delete.execute([stale.id.0])?;
            }
            for id in unreadable {
                delete.execute([id])?;
            }
        }
        transaction.commit()?;
        Ok(scheduled)
    }

    pub fn entries(&self, root: RootId) -> Result<Vec<Entry>> {
        let mut statement = self.connection.prepare_cached(&format!(
            "SELECT {COLUMNS} FROM journal WHERE root = ?1 ORDER BY id"
        ))?;
        let rows = statement.query_map([root.0], read_entry)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.extend(row?.ok());
        }
        Ok(entries)
    }

    pub fn interrupted_entries(&self) -> Result<Vec<Entry>> {
        let mut statement = self.connection.prepare_cached(&format!(
            "SELECT {COLUMNS} FROM journal WHERE running = 1 ORDER BY id"
        ))?;
        let rows = statement.query_map([], read_entry)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.extend(row?.ok());
        }
        Ok(entries)
    }

    pub fn begin(&self, entry: EntryId, artifact: Option<&Path>) -> Result<()> {
        self.connection.execute(
            "UPDATE journal SET running = 1, artifact = ?2 WHERE id = ?1",
            params![
                entry.0,
                artifact.map(|path| path.as_os_str().as_bytes().to_vec())
            ],
        )?;
        Ok(())
    }

    pub fn record_artifact(&self, entry: EntryId, artifact: &Path) -> Result<()> {
        self.connection.execute(
            "UPDATE journal SET artifact = ?2 WHERE id = ?1",
            params![entry.0, artifact.as_os_str().as_bytes()],
        )?;
        Ok(())
    }

    pub fn release(&self, entry: EntryId) -> Result<()> {
        self.connection.execute(
            "UPDATE journal SET running = 0, artifact = NULL WHERE id = ?1",
            [entry.0],
        )?;
        Ok(())
    }

    pub fn fail(&self, entry: EntryId, error: &str, retry_at: Timestamp) -> Result<u32> {
        Ok(self.connection.query_row(
            "UPDATE journal SET running = 0, artifact = NULL, attempts = attempts + 1,
             error = ?2, retry_at = ?3 WHERE id = ?1 RETURNING attempts",
            params![entry.0, error, retry_at.as_millisecond()],
            |row| row.get(0),
        )?)
    }

    pub fn finish(&mut self, root: RootId, entry: EntryId, changes: &Changes) -> Result<()> {
        let transaction = self.connection.transaction()?;
        write_changes(&transaction, root, changes)?;
        transaction.execute("DELETE FROM journal WHERE id = ?1", [entry.0])?;
        transaction.commit()?;
        Ok(())
    }
}

fn read_entry(row: &Row<'_>) -> rusqlite::Result<Result<Entry>> {
    let text: String = row.get(2)?;
    let retry_at: i64 = row.get(5)?;
    let operation = match serde_json::from_str(&text) {
        Ok(operation) => operation,
        Err(error) => return Ok(Err(error.into())),
    };
    Ok(Ok(Entry {
        id: EntryId(row.get(0)?),
        root: RootId(row.get(1)?),
        operation,
        running: row.get::<_, i64>(3)? != 0,
        attempts: row.get(4)?,
        retry_at: (retry_at > 0)
            .then(|| Timestamp::from_millisecond(retry_at).ok())
            .flatten(),
        error: row.get(6)?,
        artifact: optional_path(row.get(7)?),
    }))
}
