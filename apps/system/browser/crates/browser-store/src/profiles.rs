use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{Error, Item, Result};
use crate::favicons;
use crate::ids::ProfileId;
use crate::store::Store;
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
    pub created_at: Timestamp,
}

impl Store {
    pub fn profiles(&self) -> Result<Vec<Profile>> {
        self.read(|connection| {
            let mut statement = connection
                .prepare_cached("SELECT id, name, created_at FROM profiles ORDER BY id")?;
            let rows = statement.query_map([], |row| {
                Ok(Profile {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: row.get(2)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    pub fn create_profile(&self, name: impl Into<String>) -> ProfileId {
        let id = ProfileId::from_raw(self.sequences().profile.next());
        let name = name.into();
        let created_at = Timestamp::now();
        self.write(move |connection, _| {
            connection
                .prepare_cached("INSERT INTO profiles (id, name, created_at) VALUES (?1, ?2, ?3)")?
                .execute(params![id, name, created_at])?;
            Ok(())
        });
        id
    }

    pub fn rename_profile(&self, id: ProfileId, name: impl Into<String>) {
        let name = name.into();
        self.write(move |connection, _| {
            connection
                .prepare_cached("UPDATE profiles SET name = ?2 WHERE id = ?1")?
                .execute(params![id, name])?;
            Ok(())
        });
    }

    pub fn remove_profile(&self, id: ProfileId) -> Result<()> {
        self.call(move |connection, state| {
            remove(connection, id)?;
            state.session.invalidate();
            Ok(())
        })
    }
}

fn remove(connection: &Connection, id: ProfileId) -> Result<()> {
    let exists = connection
        .query_row("SELECT 1 FROM profiles WHERE id = ?1", [id], |_| Ok(()))
        .optional()?
        .is_some();
    if !exists {
        return Err(Error::Missing(Item::Profile(id)));
    }
    let used = connection
        .query_row(
            "SELECT 1 FROM spaces WHERE profile_id = ?1 LIMIT 1",
            [id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if used {
        return Err(Error::ProfileInUse(id));
    }
    let count: i64 = connection.query_row("SELECT count(*) FROM profiles", [], |row| row.get(0))?;
    if count <= 1 {
        return Err(Error::LastProfile);
    }
    connection.execute("DELETE FROM profiles WHERE id = ?1", [id])?;
    favicons::collect_garbage(connection)
}
