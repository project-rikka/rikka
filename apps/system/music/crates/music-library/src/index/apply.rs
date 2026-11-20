use std::path::{Path, PathBuf};

use rusqlite::types::FromSql;
use rusqlite::{OptionalExtension, Params, Transaction, params};

use crate::error::Result;
use crate::ids::TrackId;
use crate::index::record::{Record, Stat};
use crate::model::Summary;
use crate::paths;
use music_core::fold;

pub(crate) const ROLE_ARTIST: i64 = 0;
pub(crate) const ROLE_ALBUM_ARTIST: i64 = 1;
pub(crate) const ROLE_GENRE: i64 = 2;

const DURATION_SLACK_MS: i64 = 1000;
const ADOPTION_WINDOW_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Default)]
pub(crate) struct Changes {
    pub(crate) root: PathBuf,
    pub(crate) gone: Vec<TrackId>,
    pub(crate) updated: Vec<(TrackId, Record)>,
    pub(crate) added: Vec<Record>,
    pub(crate) failed: Vec<Stat>,
    pub(crate) recovered: Vec<PathBuf>,
}

impl Changes {
    pub(crate) fn new(root: PathBuf) -> Changes {
        Changes {
            root,
            ..Changes::default()
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.gone.is_empty()
            && self.updated.is_empty()
            && self.added.is_empty()
            && self.failed.is_empty()
            && self.recovered.is_empty()
    }
}

pub(crate) fn apply(transaction: &Transaction<'_>, changes: &Changes, now: i64) -> Result<Summary> {
    let mut summary = Summary::default();
    for id in &changes.gone {
        if remove(transaction, *id, now)? {
            summary.moved += 1;
        } else {
            summary.removed += 1;
        }
    }
    for (id, record) in &changes.updated {
        save_identity(transaction, *id, record)?;
        run(
            transaction,
            "DELETE FROM search WHERE rowid = ?1",
            [id.get()],
        )?;
        run(transaction, "DELETE FROM tracks WHERE id = ?1", [id.get()])?;
        insert(transaction, *id, &changes.root, record)?;
        summary.updated += 1;
    }
    for record in &changes.added {
        match place(transaction, record, now)? {
            Placed::Inserted(id) => {
                insert(transaction, id, &changes.root, record)?;
                summary.added += 1;
            }
            Placed::Moved(id) => {
                insert(transaction, id, &changes.root, record)?;
                summary.moved += 1;
            }
            Placed::Duplicate => {}
        }
    }
    for stat in &changes.failed {
        run(
            transaction,
            "INSERT OR REPLACE INTO failures (path, size, modified) VALUES (?1, ?2, ?3)",
            params![paths::bytes(&stat.path), stat.size, stat.modified],
        )?;
    }
    for path in &changes.recovered {
        run(
            transaction,
            "DELETE FROM failures WHERE path = ?1",
            [paths::bytes(path)],
        )?;
    }
    Ok(summary)
}

pub(crate) fn forget_root(transaction: &Transaction<'_>, root: &Path, now: i64) -> Result<()> {
    let root = paths::bytes(root);
    transaction.execute(
        "UPDATE identities SET gone_at = ?2 WHERE id IN (SELECT id FROM tracks WHERE root = ?1)",
        params![root, now],
    )?;
    transaction.execute(
        "DELETE FROM search WHERE rowid IN (SELECT id FROM tracks WHERE root = ?1)",
        [root],
    )?;
    transaction.execute("DELETE FROM tracks WHERE root = ?1", [root])?;
    transaction.execute("DELETE FROM roots WHERE path = ?1", [root])?;
    Ok(())
}

pub(crate) fn retire_unseen(transaction: &Transaction<'_>, root: &Path, now: i64) -> Result<()> {
    let (low, high) = paths::beneath(root);
    transaction.execute(
        "UPDATE identities SET gone_at = ?3
         WHERE gone_at IS NULL AND path >= ?1 AND path < ?2
           AND id NOT IN (SELECT id FROM tracks)",
        params![low, high, now],
    )?;
    Ok(())
}

pub(crate) fn prune_identities(transaction: &Transaction<'_>, before: i64) -> Result<()> {
    transaction.execute(
        "DELETE FROM identities
         WHERE gone_at IS NOT NULL AND gone_at < ?1
           AND id NOT IN (SELECT track FROM listening)",
        [before],
    )?;
    Ok(())
}

fn remove(transaction: &Transaction<'_>, id: TrackId, now: i64) -> Result<bool> {
    run(
        transaction,
        "DELETE FROM search WHERE rowid = ?1",
        [id.get()],
    )?;
    run(transaction, "DELETE FROM tracks WHERE id = ?1", [id.get()])?;
    run(
        transaction,
        "UPDATE identities SET gone_at = ?2 WHERE id = ?1",
        [id.get(), now],
    )?;
    let newer: Option<i64> = first(
        transaction,
        "SELECT n.id FROM identities o
         JOIN identities n ON n.size = o.size AND n.print = o.print
            AND abs(n.duration_ms - o.duration_ms) <= ?3
            AND n.id != o.id AND n.gone_at IS NULL AND n.added_at >= ?2
         WHERE o.id = ?1
           AND n.id IN (SELECT id FROM tracks)
           AND n.id NOT IN (SELECT track FROM listening)
         ORDER BY n.added_at DESC LIMIT 1",
        params![id.get(), now - ADOPTION_WINDOW_MS, DURATION_SLACK_MS],
    )?;
    let Some(newer) = newer else {
        return Ok(false);
    };
    run(
        transaction,
        "UPDATE identities SET
            (path, device, inode, size, duration_ms, print, gone_at) =
            (SELECT path, device, inode, size, duration_ms, print, NULL
             FROM identities WHERE id = ?2)
         WHERE id = ?1",
        [id.get(), newer],
    )?;
    run(transaction, "DELETE FROM identities WHERE id = ?1", [newer])?;
    run(
        transaction,
        "UPDATE tracks SET id = ?1 WHERE id = ?2",
        [id.get(), newer],
    )?;
    let words: Option<String> = first(
        transaction,
        "SELECT words FROM search WHERE rowid = ?1",
        [newer],
    )?;
    run(transaction, "DELETE FROM search WHERE rowid = ?1", [newer])?;
    if let Some(words) = words {
        run(
            transaction,
            "INSERT INTO search (rowid, words) VALUES (?1, ?2)",
            params![id.get(), words],
        )?;
    }
    Ok(true)
}

enum Placed {
    Inserted(TrackId),
    Moved(TrackId),
    Duplicate,
}

fn place(transaction: &Transaction<'_>, record: &Record, now: i64) -> Result<Placed> {
    let stat = &record.stat;
    let path = paths::bytes(&stat.path);
    let same_path: Option<i64> = first(
        transaction,
        "SELECT id FROM identities
         WHERE path = ?1 AND id NOT IN (SELECT id FROM tracks)
         ORDER BY gone_at IS NOT NULL, gone_at DESC LIMIT 1",
        [path],
    )?;
    if let Some(id) = same_path {
        let id = TrackId::new(id);
        save_identity(transaction, id, record)?;
        return Ok(Placed::Inserted(id));
    }
    let aliases: Vec<Vec<u8>> = transaction
        .prepare_cached("SELECT path FROM tracks WHERE device = ?1 AND inode = ?2")?
        .query_map([stat.device, stat.inode], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    if aliases
        .into_iter()
        .any(|alias| paths::from_bytes(alias).exists())
    {
        return Ok(Placed::Duplicate);
    }
    let moved: Option<i64> = first(
        transaction,
        "SELECT id FROM identities
         WHERE gone_at IS NOT NULL AND device = ?1 AND inode = ?2 AND size = ?3
         ORDER BY gone_at DESC LIMIT 1",
        [stat.device, stat.inode, stat.size],
    )?;
    let moved = match moved {
        Some(id) => Some(id),
        None => first(
            transaction,
            "SELECT id FROM identities
             WHERE gone_at IS NOT NULL AND size = ?1 AND print = ?2
               AND abs(duration_ms - ?3) <= ?4
             ORDER BY gone_at DESC LIMIT 1",
            params![
                stat.size,
                record.print(),
                record.duration_ms(),
                DURATION_SLACK_MS
            ],
        )?,
    };
    if let Some(id) = moved {
        let id = TrackId::new(id);
        save_identity(transaction, id, record)?;
        return Ok(Placed::Moved(id));
    }
    run(
        transaction,
        "INSERT INTO identities (path, device, inode, size, duration_ms, print, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            path,
            stat.device,
            stat.inode,
            stat.size,
            record.duration_ms(),
            record.print(),
            now
        ],
    )?;
    Ok(Placed::Inserted(TrackId::new(
        transaction.last_insert_rowid(),
    )))
}

fn save_identity(transaction: &Transaction<'_>, id: TrackId, record: &Record) -> Result<()> {
    let stat = &record.stat;
    run(
        transaction,
        "UPDATE identities SET path = ?2, device = ?3, inode = ?4, size = ?5,
            duration_ms = ?6, print = ?7, gone_at = NULL
         WHERE id = ?1",
        params![
            id.get(),
            paths::bytes(&stat.path),
            stat.device,
            stat.inode,
            stat.size,
            record.duration_ms(),
            record.print()
        ],
    )?;
    Ok(())
}

fn insert(transaction: &Transaction<'_>, id: TrackId, root: &Path, record: &Record) -> Result<()> {
    let stat = &record.stat;
    let tags = &record.tags;
    let audio = &record.audio;
    let title = record.title();
    let folder = stat.path.parent().unwrap_or(Path::new("/"));
    let file_name = stat
        .path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let artist = tags.artists.join(", ");
    let album_artist = (!tags.album_artists.is_empty()).then(|| tags.album_artists.join(", "));
    let composer = (!tags.composers.is_empty()).then(|| tags.composers.join(", "));
    let track_gain = tags.gain.track.map(|gain| gain.db);
    let track_peak = tags.gain.track.and_then(|gain| gain.peak);
    let album_gain = tags.gain.album.map(|gain| gain.db);
    let album_peak = tags.gain.album.and_then(|gain| gain.peak);
    if let Some(cover) = &record.cover {
        run(
            transaction,
            "INSERT OR IGNORE INTO artwork (hash, extension, width) VALUES (?1, ?2, ?3)",
            params![cover.hash, cover.extension, cover.width],
        )?;
    }
    transaction
        .prepare_cached(
            "INSERT INTO tracks (
                id, root, path, folder, file_name, device, inode, size, modified,
                title, artist, album, album_artist,
                track_number, track_total, disc_number, disc_total, disc_subtitle,
                year, original_year, composer, isrc,
                mb_recording, mb_track, mb_release, mb_release_group,
                label, catalogue, compilation,
                sort_title, sort_artist, sort_album, sort_album_artist,
                track_gain, track_peak, album_gain, album_peak,
                codec, lossless, duration_ms, sample_rate, bit_depth, channels, bitrate,
                cover, match_title
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17, ?18,
                ?19, ?20, ?21, ?22,
                ?23, ?24, ?25, ?26,
                ?27, ?28, ?29,
                ?30, ?31, ?32, ?33,
                ?34, ?35, ?36, ?37,
                ?38, ?39, ?40, ?41, ?42, ?43, ?44,
                ?45, ?46
            )",
        )?
        .execute(params![
            id.get(),
            paths::bytes(root),
            paths::bytes(&stat.path),
            paths::bytes(folder),
            file_name,
            stat.device,
            stat.inode,
            stat.size,
            stat.modified,
            title,
            artist,
            tags.album,
            album_artist,
            tags.track.number,
            tags.track.total,
            tags.disc.number,
            tags.disc.total,
            tags.disc_subtitle,
            tags.date.map(|date| date.year),
            tags.original_date.map(|date| date.year),
            composer,
            tags.isrc,
            tags.musicbrainz.recording,
            tags.musicbrainz.track,
            tags.musicbrainz.release,
            tags.musicbrainz.release_group,
            tags.label,
            tags.catalogue_number,
            tags.compilation,
            tags.sort.title,
            tags.sort.artist,
            tags.sort.album,
            tags.sort.album_artist,
            track_gain,
            track_peak,
            album_gain,
            album_peak,
            audio.codec.as_str(),
            audio.lossless,
            record.duration_ms(),
            audio.sample_rate,
            audio.bit_depth,
            audio.channels,
            audio.bitrate,
            record.cover.as_ref().map(|cover| cover.hash.as_str()),
            fold(&title),
        ])?;
    let mut names = transaction.prepare_cached(
        "INSERT OR IGNORE INTO track_names (track, role, position, name) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (role, values) in [
        (ROLE_ARTIST, &tags.artists),
        (ROLE_ALBUM_ARTIST, &tags.album_artists),
        (ROLE_GENRE, &tags.genres),
    ] {
        for (position, name) in (0i64..).zip(values.iter()) {
            names.execute(params![id.get(), role, position, name])?;
        }
    }
    let words = [
        title.as_str(),
        artist.as_str(),
        tags.album.as_deref().unwrap_or_default(),
        album_artist.as_deref().unwrap_or_default(),
        composer.as_deref().unwrap_or_default(),
    ]
    .map(fold)
    .join(" | ");
    run(
        transaction,
        "INSERT INTO search (rowid, words) VALUES (?1, ?2)",
        params![id.get(), words],
    )?;
    run(
        transaction,
        "DELETE FROM failures WHERE path = ?1",
        [paths::bytes(&stat.path)],
    )?;
    Ok(())
}

fn run(transaction: &Transaction<'_>, sql: &str, values: impl Params) -> Result<()> {
    transaction.prepare_cached(sql)?.execute(values)?;
    Ok(())
}

fn first<T: FromSql>(
    transaction: &Transaction<'_>,
    sql: &str,
    values: impl Params,
) -> Result<Option<T>> {
    Ok(transaction
        .prepare_cached(sql)?
        .query_row(values, |row| row.get(0))
        .optional()?)
}
