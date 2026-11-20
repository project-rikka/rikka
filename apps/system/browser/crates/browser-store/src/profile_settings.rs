use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};
use crate::ids::ProfileId;
use crate::store::Store;
use crate::writer::Coalesce;

impl Store {
    pub fn profile_settings<T: DeserializeOwned>(&self, profile: ProfileId) -> Result<Option<T>> {
        let document: Option<String> = self.read(|connection| {
            Ok(connection
                .prepare_cached("SELECT document FROM profile_settings WHERE profile_id = ?1")?
                .query_row([profile], |row| row.get(0))
                .optional()?)
        })?;
        document
            .map(|document| {
                serde_json::from_str(&document).map_err(|error| Error::MalformedSettings {
                    profile,
                    reason: error.to_string(),
                })
            })
            .transpose()
    }

    pub fn set_profile_settings<T: Serialize>(
        &self,
        profile: ProfileId,
        settings: &T,
    ) -> Result<()> {
        let document =
            serde_json::to_string(settings).map_err(|error| Error::MalformedSettings {
                profile,
                reason: error.to_string(),
            })?;
        self.write_behind(
            Some(Coalesce::ProfileSettings(profile)),
            move |connection, _| {
                connection
                    .prepare_cached(
                        "INSERT INTO profile_settings (profile_id, document) VALUES (?1, ?2) \
                         ON CONFLICT (profile_id) DO UPDATE SET document = excluded.document",
                    )?
                    .execute(params![profile, document])?;
                Ok(())
            },
        );
        Ok(())
    }
}
