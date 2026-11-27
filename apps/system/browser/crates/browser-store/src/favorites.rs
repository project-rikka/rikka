use rusqlite::params;

use crate::error::Result;
use crate::ids::{FavoriteId, ProfileId};
use crate::ordering::Ordered;
use crate::store::Store;

fn in_profile(profile: ProfileId) -> Ordered {
    Ordered {
        table: "favorites",
        scope: Some(("profile_id", profile.get())),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Favorite {
    pub id: FavoriteId,
    pub profile: ProfileId,
    pub url: String,
    pub title: String,
}

impl Store {
    pub fn favorites(&self, profile: ProfileId) -> Result<Vec<Favorite>> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT id, url, title FROM favorites WHERE profile_id = ?1 ORDER BY position, id",
            )?;
            let rows = statement.query_map([profile], |row| {
                Ok(Favorite {
                    id: row.get(0)?,
                    profile,
                    url: row.get(1)?,
                    title: row.get(2)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    pub fn add_favorite(
        &self,
        profile: ProfileId,
        url: impl Into<String>,
        title: impl Into<String>,
    ) -> FavoriteId {
        let id = FavoriteId::from_raw(self.sequences().favorite.next());
        let (url, title) = (url.into(), title.into());
        self.write(move |connection, _| {
            let position = in_profile(profile).next_position(connection)?;
            connection
                .prepare_cached(
                    "INSERT INTO favorites (id, profile_id, url, title, position) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )?
                .execute(params![id, profile, url, title, position])?;
            Ok(())
        });
        id
    }

    pub fn update_favorite(
        &self,
        id: FavoriteId,
        url: impl Into<String>,
        title: impl Into<String>,
    ) {
        let (url, title) = (url.into(), title.into());
        self.write(move |connection, _| {
            connection
                .prepare_cached("UPDATE favorites SET url = ?2, title = ?3 WHERE id = ?1")?
                .execute(params![id, url, title])?;
            Ok(())
        });
    }

    pub fn remove_favorite(&self, id: FavoriteId) {
        self.write(move |connection, state| {
            connection.execute("DELETE FROM favorites WHERE id = ?1", [id])?;
            state.session.invalidate();
            Ok(())
        });
    }

    pub fn reorder_favorites(&self, profile: ProfileId, order: &[FavoriteId]) {
        let order: Vec<i64> = order.iter().map(|id| id.get()).collect();
        self.write(move |connection, _| in_profile(profile).reorder(connection, &order));
    }
}
