use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{Error, Item, Result};
use crate::ids::{FolderId, PinnedTabId, SpaceId};
use crate::store::Store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedFolder {
    pub id: FolderId,
    pub space: SpaceId,
    pub parent: Option<FolderId>,
    pub name: String,
    pub expanded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedTab {
    pub id: PinnedTabId,
    pub space: SpaceId,
    pub folder: Option<FolderId>,
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinnedNode {
    Folder {
        folder: PinnedFolder,
        children: Vec<PinnedNode>,
    },
    Tab(PinnedTab),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinnedItem {
    Folder(FolderId),
    Tab(PinnedTabId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinnedParent {
    Root(SpaceId),
    Folder(FolderId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PinnedPlace {
    pub parent: PinnedParent,
    pub index: Option<usize>,
}

impl PinnedPlace {
    #[must_use]
    pub const fn root(space: SpaceId) -> Self {
        Self {
            parent: PinnedParent::Root(space),
            index: None,
        }
    }

    #[must_use]
    pub const fn folder(folder: FolderId) -> Self {
        Self {
            parent: PinnedParent::Folder(folder),
            index: None,
        }
    }

    #[must_use]
    pub const fn at(self, index: usize) -> Self {
        Self {
            index: Some(index),
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FolderRemoval {
    WithContents,
    KeepContents,
}

impl Store {
    pub fn pinned(&self, space: SpaceId) -> Result<Vec<PinnedNode>> {
        self.read(|connection| tree(connection, space))
    }

    pub fn create_pinned_folder(&self, place: PinnedPlace, name: impl Into<String>) -> FolderId {
        let id = FolderId::from_raw(self.sequences().folder.next());
        let name = name.into();
        self.write(move |connection, _| {
            let (space, parent) = resolve(connection, place.parent)?;
            connection
                .prepare_cached(
                    "INSERT INTO pinned_folders (id, space_id, parent_id, name, expanded, position) \
                     VALUES (?1, ?2, ?3, ?4, 1, -1)",
                )?
                .execute(params![id, space, parent, name])?;
            insert(connection, PinnedItem::Folder(id), space, parent, place.index)
        });
        id
    }

    pub fn pin_tab(
        &self,
        place: PinnedPlace,
        url: impl Into<String>,
        title: Option<String>,
    ) -> PinnedTabId {
        let id = PinnedTabId::from_raw(self.sequences().pinned_tab.next());
        let url = url.into();
        self.write(move |connection, _| {
            let (space, folder) = resolve(connection, place.parent)?;
            connection
                .prepare_cached(
                    "INSERT INTO pinned_tabs (id, space_id, folder_id, url, title, position) \
                     VALUES (?1, ?2, ?3, ?4, ?5, -1)",
                )?
                .execute(params![id, space, folder, url, title])?;
            insert(connection, PinnedItem::Tab(id), space, folder, place.index)
        });
        id
    }

    pub fn rename_pinned_folder(&self, id: FolderId, name: impl Into<String>) {
        let name = name.into();
        self.write(move |connection, _| {
            connection
                .prepare_cached("UPDATE pinned_folders SET name = ?2 WHERE id = ?1")?
                .execute(params![id, name])?;
            Ok(())
        });
    }

    pub fn update_pinned_tab(
        &self,
        id: PinnedTabId,
        url: impl Into<String>,
        title: Option<String>,
    ) {
        let url = url.into();
        self.write(move |connection, _| {
            connection
                .prepare_cached("UPDATE pinned_tabs SET url = ?2, title = ?3 WHERE id = ?1")?
                .execute(params![id, url, title])?;
            Ok(())
        });
    }

    pub fn set_pinned_folder_expanded(&self, id: FolderId, expanded: bool) {
        self.write(move |connection, _| {
            connection
                .prepare_cached("UPDATE pinned_folders SET expanded = ?2 WHERE id = ?1")?
                .execute(params![id, expanded])?;
            Ok(())
        });
    }

    pub fn move_pinned(&self, item: PinnedItem, place: PinnedPlace) {
        self.write(move |connection, _| relocate(connection, item, place));
    }

    pub fn unpin_tab(&self, id: PinnedTabId) {
        self.write(move |connection, state| {
            connection.execute("DELETE FROM pinned_tabs WHERE id = ?1", [id])?;
            state.session.invalidate();
            Ok(())
        });
    }

    pub fn remove_pinned_folder(&self, id: FolderId, removal: FolderRemoval) {
        self.write(move |connection, state| {
            if removal == FolderRemoval::KeepContents {
                release_children(connection, id)?;
            }
            connection.execute("DELETE FROM pinned_folders WHERE id = ?1", [id])?;
            state.session.invalidate();
            Ok(())
        });
    }
}

fn tree(connection: &Connection, space: SpaceId) -> Result<Vec<PinnedNode>> {
    let mut levels: HashMap<Option<FolderId>, Vec<(i64, PinnedNode)>> = HashMap::new();
    let mut folders = connection.prepare_cached(
        "SELECT id, parent_id, name, expanded, position FROM pinned_folders WHERE space_id = ?1",
    )?;
    let rows = folders.query_map([space], |row| {
        let folder = PinnedFolder {
            id: row.get(0)?,
            space,
            parent: row.get(1)?,
            name: row.get(2)?,
            expanded: row.get(3)?,
        };
        Ok((row.get::<_, i64>(4)?, folder))
    })?;
    for row in rows {
        let (position, folder) = row?;
        levels.entry(folder.parent).or_default().push((
            position,
            PinnedNode::Folder {
                folder,
                children: Vec::new(),
            },
        ));
    }
    let mut tabs = connection.prepare_cached(
        "SELECT id, folder_id, url, title, position FROM pinned_tabs WHERE space_id = ?1",
    )?;
    let rows = tabs.query_map([space], |row| {
        let tab = PinnedTab {
            id: row.get(0)?,
            space,
            folder: row.get(1)?,
            url: row.get(2)?,
            title: row.get(3)?,
        };
        Ok((row.get::<_, i64>(4)?, tab))
    })?;
    for row in rows {
        let (position, tab) = row?;
        levels
            .entry(tab.folder)
            .or_default()
            .push((position, PinnedNode::Tab(tab)));
    }
    for level in levels.values_mut() {
        level.sort_by_key(|(position, node)| (*position, rank(node)));
    }
    Ok(assemble(&mut levels, None))
}

fn rank(node: &PinnedNode) -> (u8, i64) {
    match node {
        PinnedNode::Folder { folder, .. } => (0, folder.id.get()),
        PinnedNode::Tab(tab) => (1, tab.id.get()),
    }
}

fn assemble(
    levels: &mut HashMap<Option<FolderId>, Vec<(i64, PinnedNode)>>,
    parent: Option<FolderId>,
) -> Vec<PinnedNode> {
    let level = levels.remove(&parent).unwrap_or_default();
    level
        .into_iter()
        .map(|(_, node)| match node {
            PinnedNode::Folder { folder, .. } => {
                let children = assemble(levels, Some(folder.id));
                PinnedNode::Folder { folder, children }
            }
            tab => tab,
        })
        .collect()
}

fn resolve(connection: &Connection, parent: PinnedParent) -> Result<(SpaceId, Option<FolderId>)> {
    match parent {
        PinnedParent::Root(space) => Ok((space, None)),
        PinnedParent::Folder(folder) => {
            let space = connection
                .prepare_cached("SELECT space_id FROM pinned_folders WHERE id = ?1")?
                .query_row([folder], |row| row.get(0))
                .optional()?
                .ok_or(Error::Missing(Item::Folder(folder)))?;
            Ok((space, Some(folder)))
        }
    }
}

fn siblings(
    connection: &Connection,
    space: SpaceId,
    folder: Option<FolderId>,
) -> Result<Vec<PinnedItem>> {
    let mut statement = connection.prepare_cached(
        "SELECT 0, id, position FROM pinned_folders WHERE space_id = ?1 AND parent_id IS ?2 \
         UNION ALL \
         SELECT 1, id, position FROM pinned_tabs WHERE space_id = ?1 AND folder_id IS ?2 \
         ORDER BY 3, 1, 2",
    )?;
    let rows = statement.query_map(params![space, folder], |row| {
        let id: i64 = row.get(1)?;
        Ok(match row.get::<_, i64>(0)? {
            0 => PinnedItem::Folder(FolderId::from_raw(id)),
            _ => PinnedItem::Tab(PinnedTabId::from_raw(id)),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn arrange(
    connection: &Connection,
    items: &[PinnedItem],
    space: SpaceId,
    parent: Option<FolderId>,
) -> Result<()> {
    let mut folders = connection.prepare_cached(
        "UPDATE pinned_folders SET position = ?2, space_id = ?3, parent_id = ?4 WHERE id = ?1",
    )?;
    let mut tabs = connection.prepare_cached(
        "UPDATE pinned_tabs SET position = ?2, space_id = ?3, folder_id = ?4 WHERE id = ?1",
    )?;
    for (position, item) in (0_i64..).zip(items) {
        match item {
            PinnedItem::Folder(id) => folders.execute(params![id, position, space, parent])?,
            PinnedItem::Tab(id) => tabs.execute(params![id, position, space, parent])?,
        };
    }
    Ok(())
}

fn insert(
    connection: &Connection,
    item: PinnedItem,
    space: SpaceId,
    parent: Option<FolderId>,
    index: Option<usize>,
) -> Result<()> {
    let mut items = siblings(connection, space, parent)?;
    items.retain(|sibling| *sibling != item);
    let index = index.unwrap_or(items.len()).min(items.len());
    items.insert(index, item);
    arrange(connection, &items, space, parent)
}

fn relocate(connection: &Connection, item: PinnedItem, place: PinnedPlace) -> Result<()> {
    let (space, parent) = resolve(connection, place.parent)?;
    match item {
        PinnedItem::Tab(id) => {
            let known = connection
                .query_row("SELECT 1 FROM pinned_tabs WHERE id = ?1", [id], |_| Ok(()))
                .optional()?;
            known.ok_or(Error::Missing(Item::PinnedTab(id)))?;
        }
        PinnedItem::Folder(id) => {
            let current: SpaceId = connection
                .query_row(
                    "SELECT space_id FROM pinned_folders WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or(Error::Missing(Item::Folder(id)))?;
            if let Some(parent) = parent
                && within(connection, parent, id)?
            {
                return Err(Error::FolderCycle(id));
            }
            if current != space {
                move_descendants(connection, id, space)?;
            }
        }
    }
    insert(connection, item, space, parent, place.index)
}

fn within(connection: &Connection, folder: FolderId, ancestor: FolderId) -> Result<bool> {
    let row = connection
        .prepare_cached(
            "WITH RECURSIVE up(id) AS ( \
                 SELECT ?1 \
                 UNION ALL \
                 SELECT f.parent_id FROM pinned_folders f JOIN up ON f.id = up.id \
                 WHERE f.parent_id IS NOT NULL \
             ) SELECT 1 FROM up WHERE id = ?2 LIMIT 1",
        )?
        .query_row([folder, ancestor], |_| Ok(()))
        .optional()?;
    Ok(row.is_some())
}

fn move_descendants(connection: &Connection, folder: FolderId, space: SpaceId) -> Result<()> {
    const DOWN: &str = "WITH RECURSIVE down(id) AS ( \
                            SELECT ?1 \
                            UNION ALL \
                            SELECT f.id FROM pinned_folders f JOIN down ON f.parent_id = down.id \
                        ) ";
    connection
        .prepare_cached(&format!(
            "{DOWN}UPDATE pinned_folders SET space_id = ?2 WHERE id IN down"
        ))?
        .execute(params![folder, space])?;
    connection
        .prepare_cached(&format!(
            "{DOWN}UPDATE pinned_tabs SET space_id = ?2 WHERE folder_id IN down"
        ))?
        .execute(params![folder, space])?;
    Ok(())
}

fn release_children(connection: &Connection, folder: FolderId) -> Result<()> {
    let (space, parent): (SpaceId, Option<FolderId>) = connection
        .query_row(
            "SELECT space_id, parent_id FROM pinned_folders WHERE id = ?1",
            [folder],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(Error::Missing(Item::Folder(folder)))?;
    let children = siblings(connection, space, Some(folder))?;
    let mut items = siblings(connection, space, parent)?;
    if let Some(at) = items
        .iter()
        .position(|item| *item == PinnedItem::Folder(folder))
    {
        items.splice(at..=at, children);
    }
    arrange(connection, &items, space, parent)
}
