mod drafts;
mod journal;
mod schema;

use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension as _, Transaction, params};

use crate::error::{Error, Result};
use crate::item::{Kind, LocalItem, RemoteItem, SyncedItem};
use crate::root::{Root, RootId, Target};
use crate::state::{Changes, State};

pub use drafts::{DraftBlock, DraftProgress, UploadDraft};
pub use journal::{Entry, EntryId};

const BUSY_TIMEOUT: Duration = Duration::from_secs(2);
const COMPUTER: &str = "computer";
const MY_FILES: &str = "my_files";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Local,
    Remote,
}

#[derive(Debug)]
pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Store> {
        let mut connection = Connection::open(path.as_ref())?;
        connection.busy_timeout(BUSY_TIMEOUT)?;
        let _: String =
            connection.query_row("PRAGMA locking_mode = EXCLUSIVE", [], |row| row.get(0))?;
        let mode: String =
            connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(Error::Open(format!("journal mode {mode} instead of WAL")));
        }
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.pragma_update(None, "foreign_keys", true)?;
        schema::prepare(&mut connection)?;

        Ok(Store { connection })
    }

    pub fn value(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub fn set_value(&self, key: &str, value: &str) -> Result<()> {
        self.connection.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    pub fn roots(&self) -> Result<Vec<(RootId, Root)>> {
        let mut statement = self.connection.prepare(
            "SELECT id, local_path, target, remote_path, volume, share, remote_root, local_root
             FROM roots ORDER BY id",
        )?;
        let rows = statement.query_map([], |row| {
            let target: String = row.get(2)?;
            let remote_path: String = row.get(3)?;
            Ok((
                RootId(row.get(0)?),
                Root {
                    local_path: path_from(row.get(1)?),
                    target: match target.as_str() {
                        MY_FILES => Target::MyFiles(remote_path),
                        _ => Target::Computer,
                    },
                    volume: row.get(4)?,
                    share: row.get(5)?,
                    remote_root: row.get(6)?,
                    local_root: inode_from(row.get(7)?),
                },
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn add_root(&self, root: &Root) -> Result<RootId> {
        let (target, remote_path) = target_columns(&root.target);
        self.connection.execute(
            "INSERT INTO roots (local_path, target, remote_path, volume, share, remote_root, local_root)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                root.local_path.as_os_str().as_bytes(),
                target,
                remote_path,
                root.volume,
                root.share,
                root.remote_root,
                inode_column(root.local_root),
            ],
        )?;
        Ok(RootId(self.connection.last_insert_rowid()))
    }

    pub fn update_root(&self, id: RootId, root: &Root) -> Result<()> {
        let (target, remote_path) = target_columns(&root.target);
        self.connection.execute(
            "UPDATE roots SET local_path = ?2, target = ?3, remote_path = ?4, volume = ?5,
             share = ?6, remote_root = ?7, local_root = ?8 WHERE id = ?1",
            params![
                id.0,
                root.local_path.as_os_str().as_bytes(),
                target,
                remote_path,
                root.volume,
                root.share,
                root.remote_root,
                inode_column(root.local_root),
            ],
        )?;
        Ok(())
    }

    pub fn remove_root(&self, id: RootId) -> Result<()> {
        self.connection
            .execute("DELETE FROM roots WHERE id = ?1", [id.0])?;
        Ok(())
    }

    pub fn cursor(&self, volume: &str) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row(
                "SELECT event FROM cursors WHERE volume = ?1",
                [volume],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn set_cursor(&self, volume: &str, event: &str) -> Result<()> {
        self.connection.execute(
            "INSERT INTO cursors (volume, event) VALUES (?1, ?2)
             ON CONFLICT (volume) DO UPDATE SET event = excluded.event",
            [volume, event],
        )?;
        Ok(())
    }

    pub fn load(&self, id: RootId, root: &Root) -> Result<State> {
        let mut local = self.connection.prepare(
            "SELECT inode, parent, name, kind, size, modified, sha1 FROM local_items WHERE root = ?1",
        )?;
        let local: Vec<LocalItem> = local
            .query_map([id.0], |row| {
                Ok(LocalItem {
                    id: inode_from(row.get(0)?),
                    parent: inode_from(row.get(1)?),
                    name: row.get(2)?,
                    kind: Kind::from_code(row.get(3)?),
                    size: size_from(row.get(4)?),
                    modified: row.get(5)?,
                    sha1: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let mut remote = self.connection.prepare(
            "SELECT link, parent, name, kind, revision, sha1, size, modified
             FROM remote_items WHERE root = ?1",
        )?;
        let remote: Vec<RemoteItem> = remote
            .query_map([id.0], |row| {
                Ok(RemoteItem {
                    id: row.get(0)?,
                    parent: row.get(1)?,
                    name: row.get(2)?,
                    kind: Kind::from_code(row.get(3)?),
                    revision: row.get(4)?,
                    sha1: row.get(5)?,
                    size: size_from(row.get(6)?),
                    modified: row.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let mut synced = self.connection.prepare(
            "SELECT id, kind, local, local_parent, local_name, remote, remote_parent, remote_name,
             sha1, size, revision FROM synced WHERE root = ?1",
        )?;
        let synced: Vec<SyncedItem> = synced
            .query_map([id.0], |row| {
                Ok(SyncedItem {
                    id: row.get(0)?,
                    kind: Kind::from_code(row.get(1)?),
                    local: inode_from(row.get(2)?),
                    local_parent: inode_from(row.get(3)?),
                    local_name: row.get(4)?,
                    remote: row.get(5)?,
                    remote_parent: row.get(6)?,
                    remote_name: row.get(7)?,
                    sha1: row.get(8)?,
                    size: size_from(row.get(9)?),
                    revision: row.get(10)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        Ok(State::new(root.local_root, root.remote_root.clone()).with_items(local, remote, synced))
    }

    pub fn apply(&mut self, id: RootId, changes: &Changes) -> Result<()> {
        let transaction = self.connection.transaction()?;
        write_changes(&transaction, id, changes)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn clear(&mut self, id: RootId, side: Side) -> Result<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM synced WHERE root = ?1", [id.0])?;
        transaction.execute("DELETE FROM journal WHERE root = ?1", [id.0])?;
        let items = match side {
            Side::Local => "DELETE FROM local_items WHERE root = ?1",
            Side::Remote => "DELETE FROM remote_items WHERE root = ?1",
        };
        transaction.execute(items, [id.0])?;
        transaction.commit()?;
        Ok(())
    }
}

fn write_changes(transaction: &Transaction<'_>, id: RootId, changes: &Changes) -> Result<()> {
    let root = id.0;
    {
        let mut delete =
            transaction.prepare_cached("DELETE FROM local_items WHERE root = ?1 AND inode = ?2")?;
        for inode in &changes.local_removed {
            delete.execute(params![root, inode_column(*inode)])?;
        }
        let mut delete =
            transaction.prepare_cached("DELETE FROM remote_items WHERE root = ?1 AND link = ?2")?;
        for link in &changes.remote_removed {
            delete.execute(params![root, link])?;
        }
        let mut delete =
            transaction.prepare_cached("DELETE FROM synced WHERE root = ?1 AND id = ?2")?;
        for synced in &changes.synced_removed {
            delete.execute(params![root, synced])?;
        }
    }

    let mut insert = transaction.prepare_cached(
        "INSERT OR REPLACE INTO local_items (root, inode, parent, name, kind, size, modified, sha1)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    for item in &changes.local {
        insert.execute(params![
            root,
            inode_column(item.id),
            inode_column(item.parent),
            item.name,
            item.kind.code(),
            size_column(item.size),
            item.modified,
            item.sha1,
        ])?;
    }
    let mut insert = transaction.prepare_cached(
        "INSERT OR REPLACE INTO remote_items (root, link, parent, name, kind, revision, sha1, size, modified)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    for item in &changes.remote {
        insert.execute(params![
            root,
            item.id,
            item.parent,
            item.name,
            item.kind.code(),
            item.revision,
            item.sha1,
            size_column(item.size),
            item.modified,
        ])?;
    }
    let mut insert = transaction.prepare_cached(
        "INSERT OR REPLACE INTO synced (root, id, kind, local, local_parent, local_name, remote,
         remote_parent, remote_name, sha1, size, revision)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;
    for item in &changes.synced {
        insert.execute(params![
            root,
            item.id,
            item.kind.code(),
            inode_column(item.local),
            inode_column(item.local_parent),
            item.local_name,
            item.remote,
            item.remote_parent,
            item.remote_name,
            item.sha1,
            size_column(item.size),
            item.revision,
        ])?;
    }
    Ok(())
}

fn target_columns(target: &Target) -> (&'static str, &str) {
    match target {
        Target::Computer => (COMPUTER, ""),
        Target::MyFiles(path) => (MY_FILES, path.as_str()),
    }
}

fn path_from(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(OsString::from_vec(bytes))
}

pub(crate) fn optional_path(bytes: Option<Vec<u8>>) -> Option<PathBuf> {
    bytes.map(path_from)
}

fn inode_column(inode: u64) -> i64 {
    inode.cast_signed()
}

fn inode_from(column: i64) -> u64 {
    column.cast_unsigned()
}

fn size_column(size: u64) -> i64 {
    i64::try_from(size).unwrap_or(i64::MAX)
}

fn size_from(column: i64) -> u64 {
    u64::try_from(column).unwrap_or_default()
}
