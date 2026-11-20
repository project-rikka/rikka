use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{Transaction, params};

use crate::artwork::{ArtworkCache, FolderImages, Stored};
use crate::error::Result;
use crate::index::apply::{ROLE_ALBUM_ARTIST, ROLE_ARTIST, ROLE_GENRE};
use crate::index::grouping::{self, Grouped, Member};
use crate::index::record::Stat;
use crate::paths;
use crate::store::Store;

#[derive(Clone, Debug)]
struct FolderArt {
    image: PathBuf,
    size: i64,
    modified: i64,
    hash: String,
}

pub(crate) fn aggregate(
    store: &Store,
    artwork: &ArtworkCache,
    images: &mut FolderImages,
    prune: bool,
) -> Result<()> {
    let (members, known) =
        store.read(|transaction| Ok((members(transaction)?, folder_art(transaction)?)))?;
    let mut grouped = grouping::group(members);
    let mut used: Vec<(PathBuf, FolderArt)> = Vec::new();
    let mut stored_images: Vec<Stored> = Vec::new();
    for album in grouped
        .albums
        .iter_mut()
        .filter(|album| album.artwork.is_none())
    {
        for folder in &album.folders {
            if let Some((art, stored)) = resolve(folder, known.get(folder), artwork, images) {
                album.artwork = Some(art.hash.clone());
                used.push((folder.clone(), art));
                stored_images.extend(stored);
                break;
            }
        }
    }
    store.write(move |transaction| replace(transaction, &grouped, &used, &stored_images))?;
    if !prune {
        return Ok(());
    }
    for (hash, extension) in store.write(orphans)? {
        artwork.forget(&hash, &extension);
    }
    Ok(())
}

fn resolve(
    folder: &Path,
    known: Option<&FolderArt>,
    artwork: &ArtworkCache,
    images: &mut FolderImages,
) -> Option<(FolderArt, Option<Stored>)> {
    let image = images.image(folder)?;
    let metadata = std::fs::metadata(&image).ok()?;
    let stat = Stat::from_metadata(image.clone(), &metadata);
    if let Some(known) = known.filter(|known| {
        known.image == image && known.size == stat.size && known.modified == stat.modified
    }) {
        return Some((known.clone(), None));
    }
    let stored = artwork.store(&std::fs::read(&image).ok()?)?;
    Some((
        FolderArt {
            image,
            size: stat.size,
            modified: stat.modified,
            hash: stored.hash.clone(),
        },
        Some(stored),
    ))
}

fn members(transaction: &Transaction<'_>) -> Result<Vec<Member>> {
    let mut members: Vec<Member> = transaction
        .prepare(
            "SELECT t.id, t.folder, t.file_name, t.album, t.compilation, t.mb_release, t.year,
                    t.track_number, t.disc_number, t.disc_total, t.codec, t.lossless,
                    t.bit_depth, t.sample_rate, t.duration_ms, i.added_at, t.cover,
                    t.sort_album, t.sort_album_artist, t.sort_artist
             FROM tracks t JOIN identities i ON i.id = t.id",
        )?
        .query_map([], |row| {
            Ok(Member {
                id: row.get(0)?,
                folder: paths::from_bytes(row.get(1)?),
                file_name: row.get(2)?,
                album: row.get(3)?,
                compilation: row.get(4)?,
                mb_release: row.get(5)?,
                year: row.get(6)?,
                track: row.get(7)?,
                disc: row.get(8)?,
                disc_total: row.get(9)?,
                codec: row.get(10)?,
                lossless: row.get(11)?,
                bit_depth: row.get(12)?,
                sample_rate: row.get(13)?,
                duration_ms: row.get(14)?,
                added_at: row.get(15)?,
                cover: row.get(16)?,
                sort_album: row.get(17)?,
                sort_album_artist: row.get(18)?,
                sort_artist: row.get(19)?,
                ..Member::default()
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let at: HashMap<i64, usize> = members
        .iter()
        .enumerate()
        .map(|(index, member)| (member.id, index))
        .collect();
    let mut names = transaction
        .prepare("SELECT track, role, name FROM track_names ORDER BY track, role, position")?;
    let mut rows = names.query([])?;
    while let Some(row) = rows.next()? {
        let track: i64 = row.get(0)?;
        let role: i64 = row.get(1)?;
        let name: String = row.get(2)?;
        let Some(member) = at.get(&track).map(|index| &mut members[*index]) else {
            continue;
        };
        match role {
            ROLE_ARTIST => member.artists.push(name),
            ROLE_ALBUM_ARTIST => member.album_artists.push(name),
            ROLE_GENRE => member.genres.push(name),
            _ => {}
        }
    }
    Ok(members)
}

fn folder_art(transaction: &Transaction<'_>) -> Result<HashMap<PathBuf, FolderArt>> {
    transaction
        .prepare("SELECT folder, image, size, modified, hash FROM folder_art")?
        .query_map([], |row| {
            Ok((
                paths::from_bytes(row.get(0)?),
                FolderArt {
                    image: paths::from_bytes(row.get(1)?),
                    size: row.get(2)?,
                    modified: row.get(3)?,
                    hash: row.get(4)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<_>>()
        .map_err(Into::into)
}

fn replace(
    transaction: &Transaction<'_>,
    grouped: &Grouped,
    used: &[(PathBuf, FolderArt)],
    stored_images: &[Stored],
) -> Result<()> {
    transaction.execute_batch(
        "DELETE FROM albums; DELETE FROM album_artists; DELETE FROM artists;
         DELETE FROM track_artists; DELETE FROM genres; DELETE FROM track_genres;
         DELETE FROM folder_art;",
    )?;
    for stored in stored_images {
        transaction.execute(
            "INSERT OR IGNORE INTO artwork (hash, extension, width) VALUES (?1, ?2, ?3)",
            params![stored.hash, stored.extension, stored.width],
        )?;
    }
    let mut art = transaction.prepare(
        "INSERT OR REPLACE INTO folder_art (folder, image, size, modified, hash)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for (folder, image) in used {
        art.execute(params![
            paths::bytes(folder),
            paths::bytes(&image.image),
            image.size,
            image.modified,
            image.hash
        ])?;
    }
    let mut album = transaction.prepare(
        "INSERT OR REPLACE INTO albums (id, title, artist, year, compilation, tracks, discs,
            duration_ms, codec, lossless, bit_depth, sample_rate, artwork, added_at,
            sort_title, sort_artist, folded)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
    )?;
    let mut album_artist = transaction.prepare(
        "INSERT OR IGNORE INTO album_artists (album, position, artist) VALUES (?1, ?2, ?3)",
    )?;
    let mut member = transaction.prepare("UPDATE tracks SET album_id = ?1 WHERE id = ?2")?;
    for row in &grouped.albums {
        album.execute(params![
            row.id,
            row.title,
            row.artist,
            row.year,
            row.compilation,
            i64::try_from(row.tracks.len()).unwrap_or(i64::MAX),
            row.discs,
            row.duration_ms,
            row.codec,
            row.lossless,
            row.bit_depth,
            row.sample_rate,
            row.artwork,
            row.added_at,
            row.sort_title,
            row.sort_artist,
            row.folded
        ])?;
        for (position, artist) in (0i64..).zip(&row.artists) {
            album_artist.execute(params![row.id, position, artist])?;
        }
        for track in &row.tracks {
            member.execute(params![row.id, track])?;
        }
    }
    let mut artist = transaction.prepare(
        "INSERT OR IGNORE INTO artists (id, name, sort_name, folded, albums, tracks)
         VALUES (?1, ?2, ?3, ?4, 0, 0)",
    )?;
    for row in &grouped.artists {
        artist.execute(params![row.id, row.name, row.sort_name, row.folded])?;
    }
    let mut genre = transaction
        .prepare("INSERT OR IGNORE INTO genres (id, name, folded) VALUES (?1, ?2, ?3)")?;
    for row in &grouped.genres {
        genre.execute(params![row.id, row.name, row.folded])?;
    }
    let mut track_artist = transaction.prepare(
        "INSERT OR IGNORE INTO track_artists (track, position, artist) VALUES (?1, ?2, ?3)",
    )?;
    for (track, position, id) in &grouped.track_artists {
        track_artist.execute(params![track, position, id])?;
    }
    let mut track_genre =
        transaction.prepare("INSERT OR IGNORE INTO track_genres (track, genre) VALUES (?1, ?2)")?;
    for (track, id) in &grouped.track_genres {
        track_genre.execute(params![track, id])?;
    }
    transaction.execute_batch(
        "UPDATE artists SET
            albums = (SELECT count(*) FROM album_artists WHERE artist = artists.id),
            tracks = (SELECT count(DISTINCT track) FROM track_artists WHERE artist = artists.id),
            artwork = (SELECT a.artwork FROM album_artists aa JOIN albums a ON a.id = aa.album
                       WHERE aa.artist = artists.id AND a.artwork IS NOT NULL
                       ORDER BY a.year DESC LIMIT 1);",
    )?;
    Ok(())
}

fn orphans(transaction: &Transaction<'_>) -> Result<Vec<(String, String)>> {
    let orphans: Vec<(String, String)> = transaction
        .prepare(
            "SELECT hash, extension FROM artwork
             WHERE hash NOT IN (SELECT cover FROM tracks WHERE cover IS NOT NULL)
               AND hash NOT IN (SELECT artwork FROM albums WHERE artwork IS NOT NULL)",
        )?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (hash, _) in &orphans {
        transaction.execute("DELETE FROM artwork WHERE hash = ?1", [hash])?;
    }
    Ok(orphans)
}
