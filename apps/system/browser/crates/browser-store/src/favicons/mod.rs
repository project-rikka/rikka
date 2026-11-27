use std::sync::Arc;

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{Error, Result};
use crate::ids::FaviconId;
use crate::store::Store;
use crate::url;
use crate::writer::Coalesce;

pub const FAVICON_MAX_SIDE: u32 = 64;

#[derive(Clone, PartialEq, Eq)]
pub struct Picture {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Picture {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self> {
        if width > FAVICON_MAX_SIDE || height > FAVICON_MAX_SIDE {
            return Err(Error::OversizedPicture { width, height });
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4));
        match expected {
            Some(expected) if expected == rgba.len() && expected > 0 => Ok(Self {
                width,
                height,
                rgba,
            }),
            _ => Err(Error::InvalidPicture {
                width,
                height,
                expected: expected.unwrap_or(usize::MAX),
                actual: rgba.len(),
            }),
        }
    }
}

impl std::fmt::Debug for Picture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Picture")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Favicon {
    pub id: FaviconId,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

impl std::fmt::Debug for Favicon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Favicon")
            .field("id", &self.id)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl Store {
    pub fn set_favicon(&self, page_url: impl Into<String>, picture: Picture) {
        let page_url = page_url.into();
        self.write_behind(
            Some(Coalesce::Favicon(page_url.clone())),
            move |connection, _| store(connection, &page_url, picture),
        );
    }

    pub fn favicon_for_url(&self, url: &str) -> Result<Option<Favicon>> {
        self.read(|connection| match lookup(connection, url)? {
            Some(id) => load(connection, id),
            None => Ok(None),
        })
    }

    pub fn favicon(&self, id: FaviconId) -> Result<Option<Favicon>> {
        self.read(|connection| load(connection, id))
    }
}

fn lookup(connection: &Connection, url: &str) -> Result<Option<FaviconId>> {
    let page = connection
        .prepare_cached("SELECT picture_id FROM favicon_pages WHERE url = ?1")?
        .query_row([url], |row| row.get(0))
        .optional()?;
    if page.is_some() {
        return Ok(page);
    }
    let Some(host) = url::host(url) else {
        return Ok(None);
    };
    Ok(connection
        .prepare_cached("SELECT picture_id FROM favicon_hosts WHERE host = ?1")?
        .query_row([host], |row| row.get(0))
        .optional()?)
}

fn load(connection: &Connection, id: FaviconId) -> Result<Option<Favicon>> {
    Ok(connection
        .prepare_cached("SELECT width, height, rgba FROM favicon_pictures WHERE id = ?1")?
        .query_row([id], |row| {
            let rgba: Vec<u8> = row.get(2)?;
            Ok(Favicon {
                id,
                width: row.get(0)?,
                height: row.get(1)?,
                rgba: Arc::from(rgba),
            })
        })
        .optional()?)
}

fn store(connection: &Connection, page_url: &str, picture: Picture) -> Result<()> {
    let hash = hash(picture.width, picture.height, &picture.rgba);
    let mut candidates = connection.prepare_cached(
        "SELECT id FROM favicon_pictures \
         WHERE hash = ?1 AND width = ?2 AND height = ?3 AND rgba = ?4 LIMIT 1",
    )?;
    let existing: Option<FaviconId> = candidates
        .query_row(
            params![hash, picture.width, picture.height, picture.rgba],
            |row| row.get(0),
        )
        .optional()?;
    let id = match existing {
        Some(id) => id,
        None => connection
            .prepare_cached(
                "INSERT INTO favicon_pictures (hash, width, height, rgba) \
                 VALUES (?1, ?2, ?3, ?4) RETURNING id",
            )?
            .query_row(
                params![hash, picture.width, picture.height, picture.rgba],
                |row| row.get(0),
            )?,
    };
    let mut replaced = Vec::with_capacity(2);
    replaced.extend(
        connection
            .prepare_cached(
                "SELECT picture_id FROM favicon_pages WHERE url = ?1 AND picture_id IS NOT ?2",
            )?
            .query_row(params![page_url, id], |row| row.get::<_, FaviconId>(0))
            .optional()?,
    );
    connection
        .prepare_cached(
            "INSERT INTO favicon_pages (url, picture_id) VALUES (?1, ?2) \
             ON CONFLICT (url) DO UPDATE SET picture_id = excluded.picture_id",
        )?
        .execute(params![page_url, id])?;
    if let Some(host) = url::host(page_url) {
        if is_root(page_url) {
            replaced.extend(
                connection
                    .prepare_cached(
                        "SELECT picture_id FROM favicon_hosts \
                         WHERE host = ?1 AND picture_id IS NOT ?2",
                    )?
                    .query_row(params![host, id], |row| row.get::<_, FaviconId>(0))
                    .optional()?,
            );
            connection
                .prepare_cached(
                    "INSERT INTO favicon_hosts (host, picture_id) VALUES (?1, ?2) \
                     ON CONFLICT (host) DO UPDATE SET picture_id = excluded.picture_id",
                )?
                .execute(params![host, id])?;
        } else {
            connection
                .prepare_cached(
                    "INSERT OR IGNORE INTO favicon_hosts (host, picture_id) VALUES (?1, ?2)",
                )?
                .execute(params![host, id])?;
        }
    }
    let mut release = connection.prepare_cached(
        "DELETE FROM favicon_pictures WHERE id = ?1 \
         AND NOT EXISTS (SELECT 1 FROM favicon_pages WHERE picture_id = ?1) \
         AND NOT EXISTS (SELECT 1 FROM favicon_hosts WHERE picture_id = ?1)",
    )?;
    for picture in replaced {
        release.execute([picture])?;
    }
    Ok(())
}

fn is_root(url: &str) -> bool {
    url.split_once("://")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.find('/').map(|slash| &rest[slash..]))
        .is_none_or(|path| path == "/")
}

pub(crate) fn collect_garbage(connection: &Connection) -> Result<()> {
    connection
        .prepare_cached(
            "DELETE FROM favicon_pages WHERE url NOT IN ( \
                 SELECT url FROM favorites \
                 UNION SELECT url FROM pinned_tabs \
                 UNION SELECT url FROM session_tabs \
                 UNION SELECT url FROM archived_tabs \
                 UNION SELECT url FROM closed_tabs)",
        )?
        .execute([])?;
    connection
        .prepare_cached(
            "DELETE FROM favicon_hosts WHERE picture_id NOT IN (SELECT picture_id FROM favicon_pages)",
        )?
        .execute([])?;
    connection
        .prepare_cached(
            "DELETE FROM favicon_pictures WHERE id NOT IN (SELECT picture_id FROM favicon_pages) \
             AND id NOT IN (SELECT picture_id FROM favicon_hosts)",
        )?
        .execute([])?;
    Ok(())
}

fn hash(width: u32, height: u32, rgba: &[u8]) -> i64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    let hash = width
        .to_le_bytes()
        .iter()
        .chain(&height.to_le_bytes())
        .chain(rgba)
        .fold(OFFSET, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
        });
    i64::from_le_bytes(hash.to_le_bytes())
}
