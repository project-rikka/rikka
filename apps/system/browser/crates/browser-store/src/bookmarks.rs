use rusqlite::{Connection, OptionalExtension, params};

use crate::error::Result;
use crate::ids::{FavoriteId, FolderId, PinnedTabId, ProfileId, SpaceId};
use crate::store::Store;

const KIND_COLUMN: usize = 0;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BookmarkRoots {
    pub rikka: String,
    pub favorites: String,
    pub spaces: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkedItem {
    Favorite(FavoriteId),
    Space(SpaceId),
    Folder(FolderId),
    Pin(PinnedTabId),
}

impl LinkedItem {
    fn code(self) -> (i64, i64) {
        match self {
            Self::Favorite(id) => (0, id.get()),
            Self::Space(id) => (1, id.get()),
            Self::Folder(id) => (2, id.get()),
            Self::Pin(id) => (3, id.get()),
        }
    }

    fn from_code(kind: i64, id: i64) -> rusqlite::Result<Self> {
        match kind {
            0 => Ok(Self::Favorite(FavoriteId::from_raw(id))),
            1 => Ok(Self::Space(SpaceId::from_raw(id))),
            2 => Ok(Self::Folder(FolderId::from_raw(id))),
            3 => Ok(Self::Pin(PinnedTabId::from_raw(id))),
            unknown => Err(rusqlite::Error::IntegralValueOutOfRange(
                KIND_COLUMN,
                unknown,
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BookmarkLink {
    pub bookmark: String,
    pub item: LinkedItem,
    pub parent: String,
    pub position: u32,
    pub title: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkSync {
    pub roots: BookmarkRoots,
    pub links: Vec<BookmarkLink>,
}

impl Store {
    pub fn bookmark_sync(&self, profile: ProfileId) -> Result<Option<BookmarkSync>> {
        self.read(|connection| load(connection, profile))
    }

    pub fn save_bookmark_sync(&self, profile: ProfileId, sync: BookmarkSync) {
        self.write(move |connection, _| save(connection, profile, &sync));
    }
}

fn load(connection: &Connection, profile: ProfileId) -> Result<Option<BookmarkSync>> {
    let roots = connection
        .prepare_cached(
            "SELECT rikka, favorites, spaces FROM bookmark_roots WHERE profile_id = ?1",
        )?
        .query_row([profile], |row| {
            Ok(BookmarkRoots {
                rikka: row.get(0)?,
                favorites: row.get(1)?,
                spaces: row.get(2)?,
            })
        })
        .optional()?;
    let Some(roots) = roots else {
        return Ok(None);
    };
    let mut statement = connection.prepare_cached(
        "SELECT kind, local_id, bookmark, parent, position, title, url FROM bookmark_links \
         WHERE profile_id = ?1 ORDER BY parent, position",
    )?;
    let links = statement
        .query_map([profile], |row| {
            Ok(BookmarkLink {
                item: LinkedItem::from_code(row.get(0)?, row.get(1)?)?,
                bookmark: row.get(2)?,
                parent: row.get(3)?,
                position: row.get(4)?,
                title: row.get(5)?,
                url: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Some(BookmarkSync { roots, links }))
}

fn save(connection: &Connection, profile: ProfileId, sync: &BookmarkSync) -> Result<()> {
    connection
        .prepare_cached(
            "INSERT INTO bookmark_roots (profile_id, rikka, favorites, spaces) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT (profile_id) DO UPDATE SET rikka = excluded.rikka, \
             favorites = excluded.favorites, spaces = excluded.spaces",
        )?
        .execute(params![
            profile,
            sync.roots.rikka,
            sync.roots.favorites,
            sync.roots.spaces
        ])?;
    connection
        .prepare_cached("DELETE FROM bookmark_links WHERE profile_id = ?1")?
        .execute([profile])?;
    let mut insert = connection.prepare_cached(
        "INSERT OR REPLACE INTO bookmark_links \
         (profile_id, bookmark, kind, local_id, parent, position, title, url) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    for link in &sync.links {
        let (kind, local) = link.item.code();
        insert.execute(params![
            profile,
            link.bookmark,
            kind,
            local,
            link.parent,
            link.position,
            link.title,
            link.url
        ])?;
    }
    Ok(())
}
