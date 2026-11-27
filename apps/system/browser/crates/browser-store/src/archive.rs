use rusqlite::types::Value;
use rusqlite::{params, params_from_iter};

use crate::error::Result;
use crate::ids::{ArchivedTabId, ProfileId, SpaceId};
use crate::parked::ARCHIVED;
use crate::records::{Restored, TabRecord, count, like_pattern, push_like};
use crate::store::Store;
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivedTab {
    pub id: ArchivedTabId,
    pub space: SpaceId,
    pub url: String,
    pub title: String,
    pub archived_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveScope {
    Everywhere,
    Profile(ProfileId),
    Space(SpaceId),
}

impl Store {
    pub fn archive_tabs(&self, space: SpaceId, tabs: Vec<TabRecord>) -> Vec<ArchivedTabId> {
        let archived_at = Timestamp::now();
        let ids: Vec<ArchivedTabId> = tabs
            .iter()
            .map(|_| ArchivedTabId::from_raw(self.sequences().archived_tab.next()))
            .collect();
        let rows = ids.clone();
        self.write(move |connection, _| {
            let mut insert = connection.prepare_cached(
                "INSERT INTO archived_tabs (id, space_id, url, title, archived_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for (id, tab) in rows.into_iter().zip(tabs) {
                insert.execute(params![id, space, tab.url, tab.title, archived_at])?;
            }
            Ok(())
        });
        ids
    }

    pub fn search_archive(
        &self,
        scope: ArchiveScope,
        text: &str,
        limit: usize,
    ) -> Result<Vec<ArchivedTab>> {
        let mut sql =
            String::from("SELECT id, space_id, url, title, archived_at FROM archived_tabs WHERE 1");
        let mut values = Vec::new();
        match scope {
            ArchiveScope::Everywhere => {}
            ArchiveScope::Profile(profile) => {
                sql.push_str(" AND space_id IN (SELECT id FROM spaces WHERE profile_id = ?)");
                values.push(Value::Integer(profile.get()));
            }
            ArchiveScope::Space(space) => {
                sql.push_str(" AND space_id = ?");
                values.push(Value::Integer(space.get()));
            }
        }
        for term in text.split_whitespace() {
            push_like(&mut sql, &mut values, &like_pattern(term));
        }
        sql.push_str(" ORDER BY archived_at DESC, id DESC LIMIT ?");
        values.push(Value::Integer(count(limit)));
        self.read(|connection| {
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(params_from_iter(values), |row| {
                Ok(ArchivedTab {
                    id: row.get(0)?,
                    space: row.get(1)?,
                    url: row.get(2)?,
                    title: row.get(3)?,
                    archived_at: row.get(4)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    pub fn restore_archived(&self, id: ArchivedTabId) -> Result<Option<Restored>> {
        self.call(move |connection, _| ARCHIVED.take(connection, id.get()))
    }

    pub fn remove_archived(&self, id: ArchivedTabId) {
        self.write(move |connection, _| {
            connection.execute("DELETE FROM archived_tabs WHERE id = ?1", [id])?;
            Ok(())
        });
    }
}
