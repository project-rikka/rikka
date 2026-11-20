use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use rusqlite::{Connection, params};

use crate::error::Result;
use crate::ids::{FavoriteId, PinnedTabId, SpaceId, SplitGroupId, TabId};
use crate::store::Store;
use crate::time::Timestamp;

pub use split::{SessionSplit, SplitAxis};

mod split;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Session {
    pub windows: Vec<SessionWindow>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionWindow {
    pub active_space: Option<SpaceId>,
    pub bounds: Option<WindowBounds>,
    pub maximized: bool,
    pub spaces: Vec<SpaceSession>,
    pub splits: Vec<SessionSplit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowBounds {
    pub position: Option<(i32, i32)>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceSession {
    pub space: SpaceId,
    pub active: Option<TabId>,
    pub tabs: Vec<SessionTab>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionTab {
    pub id: TabId,
    pub pinned: Option<PinnedTabId>,
    pub favorite: Option<FavoriteId>,
    pub url: String,
    pub title: String,
    pub state: Option<Arc<[u8]>>,
    pub last_active: Timestamp,
    pub split: Option<SplitMember>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SplitMember {
    pub group: SplitGroupId,
    pub order: u32,
}

#[derive(Debug, Default)]
pub(crate) struct SessionCache {
    rows: Option<HashMap<TabId, u64>>,
}

impl SessionCache {
    pub(crate) fn invalidate(&mut self) {
        self.rows = None;
    }
}

impl Store {
    pub fn save_session(&self, session: Session) {
        self.queue_session(session);
    }

    pub fn load_session(&self) -> Result<Session> {
        self.read(load)
    }
}

fn load(connection: &Connection) -> Result<Session> {
    let mut statement = connection.prepare_cached(
        "SELECT active_space, x, y, width, height, maximized FROM session_windows ORDER BY position",
    )?;
    let mut windows = statement
        .query_map([], |row| {
            let x: Option<i32> = row.get(1)?;
            let y: Option<i32> = row.get(2)?;
            let width: Option<u32> = row.get(3)?;
            let height: Option<u32> = row.get(4)?;
            Ok(SessionWindow {
                active_space: row.get(0)?,
                bounds: width.zip(height).map(|(width, height)| WindowBounds {
                    position: x.zip(y),
                    width,
                    height,
                }),
                maximized: row.get(5)?,
                spaces: Vec::new(),
                splits: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut index: HashMap<(usize, SpaceId), usize> = HashMap::new();
    let mut statement = connection.prepare_cached(
        "SELECT window_index, space_id, active_tab FROM session_spaces \
         ORDER BY window_index, position",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get(1)?, row.get(2)?))
    })?;
    for row in rows {
        let (window, space, active) = row?;
        let Some(window) = usize::try_from(window)
            .ok()
            .filter(|at| *at < windows.len())
        else {
            continue;
        };
        let spaces = &mut windows[window].spaces;
        index.insert((window, space), spaces.len());
        spaces.push(SpaceSession {
            space,
            active,
            tabs: Vec::new(),
        });
    }

    let mut statement = connection.prepare_cached(
        "SELECT window_index, space_id, id, pinned_id, url, title, state, last_active, \
         split_group, split_order, favorite_id FROM session_tabs ORDER BY window_index, space_id, position, id",
    )?;
    let rows = statement.query_map([], |row| {
        let state: Option<Vec<u8>> = row.get(6)?;
        let group: Option<SplitGroupId> = row.get(8)?;
        let order: Option<u32> = row.get(9)?;
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, SpaceId>(1)?,
            SessionTab {
                id: row.get(2)?,
                pinned: row.get(3)?,
                favorite: row.get(10)?,
                url: row.get(4)?,
                title: row.get(5)?,
                state: state.map(Arc::from),
                last_active: row.get(7)?,
                split: group
                    .zip(order)
                    .map(|(group, order)| SplitMember { group, order }),
            },
        ))
    })?;
    for row in rows {
        let (window, space, tab) = row?;
        let Ok(window) = usize::try_from(window) else {
            continue;
        };
        if let Some(&at) = index.get(&(window, space)) {
            windows[window].spaces[at].tabs.push(tab);
        }
    }

    let mut statement = connection.prepare_cached(
        "SELECT window_index, split_group, axis, shares FROM session_splits \
         ORDER BY window_index, position",
    )?;
    let rows = statement.query_map([], |row| {
        let shares: Vec<u8> = row.get(3)?;
        Ok((
            row.get::<_, i64>(0)?,
            SessionSplit {
                group: row.get(1)?,
                axis: SplitAxis::from_code(row.get(2)?),
                shares: split::decode(&shares),
            },
        ))
    })?;
    let mut stored: Vec<Vec<SessionSplit>> = vec![Vec::new(); windows.len()];
    for row in rows {
        let (window, split) = row?;
        if let Some(splits) = usize::try_from(window)
            .ok()
            .and_then(|window| stored.get_mut(window))
        {
            splits.push(split);
        }
    }
    for (window, splits) in windows.iter_mut().zip(stored) {
        let valid = split::validate(&splits, &members(window.spaces.iter()));
        for tab in window
            .spaces
            .iter_mut()
            .flat_map(|space| space.tabs.iter_mut())
        {
            tab.split = grouped(tab.split, &valid);
        }
        window.splits = valid;
    }
    Ok(Session { windows })
}

fn members<'a>(spaces: impl Iterator<Item = &'a SpaceSession>) -> HashMap<SplitGroupId, usize> {
    let mut counts = HashMap::new();
    for member in spaces
        .flat_map(|space| &space.tabs)
        .filter_map(|tab| tab.split)
    {
        *counts.entry(member.group).or_insert(0) += 1;
    }
    counts
}

fn grouped(member: Option<SplitMember>, valid: &[SessionSplit]) -> Option<SplitMember> {
    member.filter(|member| valid.iter().any(|split| split.group == member.group))
}

fn fingerprint(
    window: i64,
    space: SpaceId,
    position: i64,
    tab: &SessionTab,
    split: Option<SplitMember>,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    window.hash(&mut hasher);
    space.hash(&mut hasher);
    position.hash(&mut hasher);
    tab.hash(&mut hasher);
    split.hash(&mut hasher);
    hasher.finish()
}

fn ids<T: Eq + Hash + rusqlite::types::FromSql>(
    connection: &Connection,
    sql: &str,
) -> Result<HashSet<T>> {
    let mut statement = connection.prepare_cached(sql)?;
    let rows = statement.query_map([], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

struct Placed<'a> {
    window: i64,
    space: SpaceId,
    position: i64,
    tab: &'a SessionTab,
    split: Option<SplitMember>,
}

pub(crate) fn write(
    connection: &Connection,
    cache: &mut SessionCache,
    session: &Session,
) -> Result<()> {
    let spaces: HashSet<SpaceId> = ids(connection, "SELECT id FROM spaces")?;
    let pinned: HashSet<PinnedTabId> = ids(connection, "SELECT id FROM pinned_tabs")?;
    let favorites: HashSet<FavoriteId> = ids(connection, "SELECT id FROM favorites")?;

    connection.execute("DELETE FROM session_windows", [])?;
    connection.execute("DELETE FROM session_spaces", [])?;
    connection.execute("DELETE FROM session_splits", [])?;
    let mut insert_split = connection.prepare_cached(
        "INSERT INTO session_splits (window_index, split_group, position, axis, shares) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    let mut insert_window = connection.prepare_cached(
        "INSERT INTO session_windows (position, active_space, x, y, width, height, maximized) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    let mut insert_space = connection.prepare_cached(
        "INSERT OR REPLACE INTO session_spaces (window_index, space_id, position, active_tab) \
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut rows = HashMap::new();
    let mut changed = Vec::new();
    for (window, state) in (0_i64..).zip(&session.windows) {
        let active = state.active_space.filter(|space| spaces.contains(space));
        let bounds = state.bounds;
        let point = bounds.and_then(|bounds| bounds.position);
        insert_window.execute(params![
            window,
            active,
            point.map(|(x, _)| x),
            point.map(|(_, y)| y),
            bounds.map(|bounds| bounds.width),
            bounds.map(|bounds| bounds.height),
            state.maximized
        ])?;
        let spaces_in_order = || {
            state
                .spaces
                .iter()
                .filter(|space| spaces.contains(&space.space))
        };
        let valid = split::validate(&state.splits, &members(spaces_in_order()));
        for (order, group) in (0_i64..).zip(&valid) {
            insert_split.execute(params![
                window,
                group.group,
                order,
                group.axis.code(),
                split::encode(&group.shares)
            ])?;
        }
        for (order, space) in (0_i64..).zip(spaces_in_order()) {
            insert_space.execute(params![window, space.space, order, space.active])?;
            for (position, tab) in (0_i64..).zip(&space.tabs) {
                let split = grouped(tab.split, &valid);
                let print = fingerprint(window, space.space, position, tab, split);
                let previous = cache.rows.as_ref().and_then(|rows| rows.get(&tab.id));
                if previous != Some(&print) {
                    changed.push(Placed {
                        window,
                        space: space.space,
                        position,
                        tab,
                        split,
                    });
                }
                rows.insert(tab.id, print);
            }
        }
    }

    match &cache.rows {
        Some(previous) => {
            let mut delete = connection.prepare_cached("DELETE FROM session_tabs WHERE id = ?1")?;
            for id in previous.keys().filter(|id| !rows.contains_key(id)) {
                delete.execute([id])?;
            }
        }
        None => {
            connection.execute("DELETE FROM session_tabs", [])?;
        }
    }

    let mut upsert = connection.prepare_cached(
        "INSERT OR REPLACE INTO session_tabs \
         (id, window_index, space_id, pinned_id, position, url, title, state, last_active, \
         split_group, split_order, favorite_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;
    for Placed {
        window,
        space,
        position,
        tab,
        split,
    } in changed
    {
        let pinned = tab.pinned.filter(|id| pinned.contains(id));
        let favorite = tab
            .favorite
            .filter(|id| pinned.is_none() && favorites.contains(id));
        upsert.execute(params![
            tab.id,
            window,
            space,
            pinned,
            position,
            tab.url,
            tab.title,
            tab.state.as_deref(),
            tab.last_active,
            split.map(|split| split.group),
            split.map(|split| split.order),
            favorite
        ])?;
    }

    cache.rows = Some(rows);
    Ok(())
}
