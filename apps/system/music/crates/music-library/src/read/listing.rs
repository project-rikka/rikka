use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use rusqlite::{Transaction, params};

use crate::artwork::ArtworkCache;
use crate::error::Result;
use crate::model::{Listing, Subfolder, Track};
use crate::paths;
use crate::read::tracks::{self, IN_ALBUM_ORDER};

pub(crate) fn listing(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    folder: &Path,
) -> Result<Listing> {
    let (low, high) = paths::beneath(folder);
    let counted: Vec<(Vec<u8>, u32)> = transaction
        .prepare_cached(
            "SELECT folder, count(*) FROM tracks WHERE folder >= ?1 AND folder < ?2
             GROUP BY folder",
        )?
        .query_map(params![low, high], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut children: BTreeMap<(String, String), (PathBuf, u32)> = BTreeMap::new();
    for (path, count) in counted {
        let path = paths::from_bytes(path);
        let Some(child) = path
            .strip_prefix(folder)
            .ok()
            .and_then(|inner| inner.components().next())
        else {
            continue;
        };
        let entry = children
            .entry(ordered(child))
            .or_insert_with(|| (folder.join(child), 0));
        entry.1 += count;
    }
    let folders = children
        .into_values()
        .map(|(path, tracks)| Subfolder {
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path,
            tracks,
        })
        .collect();
    let tracks = tracks::select(
        transaction,
        artwork,
        &format!("WHERE t.folder = ?1 ORDER BY {IN_ALBUM_ORDER}"),
        [paths::bytes(folder)],
    )?;
    Ok(Listing {
        path: folder.to_path_buf(),
        folders,
        tracks,
    })
}

pub(crate) fn under(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    folder: &Path,
) -> Result<Vec<Track>> {
    let (low, high) = paths::beneath(folder);
    let mut tracks = tracks::select(
        transaction,
        artwork,
        &format!(
            "WHERE t.folder = ?1 OR (t.folder >= ?2 AND t.folder < ?3) ORDER BY {IN_ALBUM_ORDER}"
        ),
        params![paths::bytes(folder), low, high],
    )?;
    tracks.sort_by_cached_key(|track| {
        track
            .path
            .parent()
            .and_then(|parent| parent.strip_prefix(folder).ok())
            .map(|inner| inner.components().map(ordered).collect::<Vec<_>>())
            .unwrap_or_default()
    });
    Ok(tracks)
}

pub(crate) fn count_under(transaction: &Transaction<'_>, folder: &Path) -> Result<u32> {
    let (low, high) = paths::beneath(folder);
    Ok(transaction
        .prepare_cached(
            "SELECT count(*) FROM tracks WHERE folder = ?1 OR (folder >= ?2 AND folder < ?3)",
        )?
        .query_row(params![paths::bytes(folder), low, high], |row| row.get(0))?)
}

fn ordered(part: Component<'_>) -> (String, String) {
    let name = part.as_os_str().to_string_lossy().into_owned();
    (name.to_lowercase(), name)
}
