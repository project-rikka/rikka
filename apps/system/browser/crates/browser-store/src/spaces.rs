use rusqlite::{Row, params};

use crate::error::Result;
use crate::ids::{ProfileId, SpaceId};
use crate::ordering::Ordered;
use crate::store::Store;

const SPACES: Ordered = Ordered {
    table: "spaces",
    scope: None,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpaceIcon {
    Emoji(String),
    Symbol(String),
}

impl SpaceIcon {
    fn kind(&self) -> i64 {
        match self {
            Self::Emoji(_) => 0,
            Self::Symbol(_) => 1,
        }
    }

    fn text(&self) -> &str {
        match self {
            Self::Emoji(text) | Self::Symbol(text) => text,
        }
    }

    fn from_parts(kind: i64, text: String) -> Self {
        match kind {
            0 => Self::Emoji(text),
            _ => Self::Symbol(text),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceFields {
    pub name: String,
    pub icon: SpaceIcon,
    pub profile: ProfileId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Space {
    pub id: SpaceId,
    pub name: String,
    pub icon: SpaceIcon,
    pub profile: ProfileId,
}

fn space(row: &Row<'_>) -> rusqlite::Result<Space> {
    Ok(Space {
        id: row.get(0)?,
        name: row.get(1)?,
        icon: SpaceIcon::from_parts(row.get(2)?, row.get(3)?),
        profile: row.get(4)?,
    })
}

impl Store {
    pub fn spaces(&self) -> Result<Vec<Space>> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT id, name, icon_kind, icon, profile_id FROM spaces \
                 ORDER BY position, id",
            )?;
            let rows = statement.query_map([], space)?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    pub fn create_space(&self, fields: SpaceFields) -> SpaceId {
        let id = SpaceId::from_raw(self.sequences().space.next());
        self.write(move |connection, _| {
            let position = SPACES.next_position(connection)?;
            connection
                .prepare_cached(
                    "INSERT INTO spaces (id, name, icon_kind, icon, profile_id, position) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )?
                .execute(params![
                    id,
                    fields.name,
                    fields.icon.kind(),
                    fields.icon.text(),
                    fields.profile,
                    position
                ])?;
            Ok(())
        });
        id
    }

    pub fn update_space(&self, id: SpaceId, fields: SpaceFields) {
        self.write(move |connection, _| {
            connection
                .prepare_cached(
                    "UPDATE spaces SET name = ?2, icon_kind = ?3, icon = ?4, profile_id = ?5 \
                     WHERE id = ?1",
                )?
                .execute(params![
                    id,
                    fields.name,
                    fields.icon.kind(),
                    fields.icon.text(),
                    fields.profile
                ])?;
            Ok(())
        });
    }

    pub fn remove_space(&self, id: SpaceId) {
        self.write(move |connection, state| {
            connection.execute("DELETE FROM spaces WHERE id = ?1", [id])?;
            state.session.invalidate();
            Ok(())
        });
    }
}
