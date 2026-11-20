use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{Transaction, params};

use crate::error::Result;
use crate::model::{Folder, FolderKind};
use crate::paths;
use crate::scan::RootState;
use crate::store::Store;

pub(crate) fn configured(store: &Store, music: &Path) -> Result<Vec<(PathBuf, FolderKind)>> {
    let added: Vec<PathBuf> = store.read(|transaction| {
        transaction
            .prepare("SELECT path FROM folders ORDER BY added_at")?
            .query_map([], |row| row.get::<_, Vec<u8>>(0))?
            .map(|path| path.map(paths::from_bytes).map_err(Into::into))
            .collect()
    })?;
    let mut folders = vec![(music.to_path_buf(), FolderKind::Music)];
    folders.extend(
        added
            .into_iter()
            .filter(|path| path != music)
            .map(|path| (path, FolderKind::Added)),
    );
    Ok(folders)
}

pub(crate) fn listed(store: &Store, music: &Path) -> Result<Vec<Folder>> {
    let states = states(store)?;
    Ok(configured(store, music)?
        .into_iter()
        .map(|(path, kind)| {
            let state = states
                .iter()
                .filter(|(root, _)| path.starts_with(root))
                .max_by_key(|(root, _)| root.as_os_str().len())
                .map(|(_, state)| *state);
            Folder {
                online: state.is_some_and(|state| state.online),
                remote: state.is_some_and(|state| state.remote),
                path,
                kind,
            }
        })
        .collect())
}

pub(crate) fn states(store: &Store) -> Result<HashMap<PathBuf, RootState>> {
    store.read(|transaction| {
        transaction
            .prepare("SELECT path, online, remote, device FROM roots")?
            .query_map([], |row| {
                Ok((
                    paths::from_bytes(row.get(0)?),
                    RootState {
                        online: row.get(1)?,
                        remote: row.get(2)?,
                        device: row.get(3)?,
                    },
                ))
            })?
            .collect::<rusqlite::Result<_>>()
            .map_err(Into::into)
    })
}

pub(crate) fn add(transaction: &Transaction<'_>, path: &Path, now: i64) -> Result<bool> {
    Ok(transaction.execute(
        "INSERT OR IGNORE INTO folders (path, added_at) VALUES (?1, ?2)",
        params![paths::bytes(path), now],
    )? > 0)
}

pub(crate) fn remove(transaction: &Transaction<'_>, path: &Path) -> Result<bool> {
    Ok(transaction.execute("DELETE FROM folders WHERE path = ?1", [paths::bytes(path)])? > 0)
}

pub(crate) fn save_states(
    transaction: &Transaction<'_>,
    states: &[(PathBuf, RootState)],
) -> Result<()> {
    let known: Vec<&[u8]> = states.iter().map(|(path, _)| paths::bytes(path)).collect();
    let listed: Vec<Vec<u8>> = transaction
        .prepare("SELECT path FROM roots")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for path in listed {
        if !known.contains(&path.as_slice()) {
            transaction.execute("DELETE FROM roots WHERE path = ?1", [&path])?;
        }
    }
    for (path, state) in states {
        transaction.execute(
            "INSERT OR REPLACE INTO roots (path, online, remote, device) VALUES (?1, ?2, ?3, ?4)",
            params![paths::bytes(path), state.online, state.remote, state.device],
        )?;
        let (low, high) = paths::beneath(path);
        transaction.execute(
            "UPDATE tracks SET root = ?1 WHERE path >= ?2 AND path < ?3 AND root != ?1",
            params![paths::bytes(path), low, high],
        )?;
    }
    Ok(())
}
