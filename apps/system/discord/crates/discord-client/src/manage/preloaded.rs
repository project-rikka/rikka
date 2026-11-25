use discord_core::{PrivacyChange, SpaceId, SpacePrivacy};
use discord_wire::{Preloaded, StringValue, UInt64Value};
use serde::Deserialize;
use serde_json::json;

use crate::client::Client;
use crate::error::{Error, Result};
use crate::session::Session;

const SETTINGS_PROTO: &str = "/users/@me/settings-proto/1";
const PRELOADED: u8 = 1;

#[derive(Deserialize)]
struct ProtoReply {
    settings: String,
}

async fn patch_proto(session: &Session, settings: String) -> Result<()> {
    let reply: ProtoReply = session
        .rest()
        .patch(SETTINGS_PROTO)
        .json(json!({ "settings": settings }))
        .parse()
        .await?;
    let event = json!({
        "settings": { "type": PRELOADED, "proto": reply.settings },
        "partial": false,
    });
    session.absorb("USER_SETTINGS_PROTO_UPDATE", event.to_string().as_bytes())
}

fn unreadable_privacy() -> Error {
    Error::Unreadable("Discord sent privacy settings that could not be read".into())
}

impl Client {
    pub async fn edit_folder(
        &self,
        folder: &str,
        name: Option<String>,
        color: Option<u32>,
    ) -> discord_core::Result<()> {
        let folder = folder.to_owned();
        self.work(move |session| async move {
            let mut folders = session.state().guild_folders();
            let entry = folders
                .folders
                .iter_mut()
                .find(|entry| {
                    entry
                        .id
                        .as_ref()
                        .is_some_and(|id| id.value.to_string() == folder)
                })
                .ok_or_else(|| Error::Missing(format!("folder {folder}")))?;
            entry.name = name
                .map(|name| name.trim().to_owned())
                .filter(|name| !name.is_empty())
                .map(|value| StringValue { value });
            entry.color = color.map(|color| UInt64Value {
                value: u64::from(color),
            });
            patch_proto(&session, Preloaded::folders_patch(folders)).await
        })
        .await
    }

    pub fn space_privacy(&self, space: &SpaceId) -> discord_core::Result<SpacePrivacy> {
        Ok(self
            .session()
            .state()
            .space_privacy(*space)
            .ok_or_else(unreadable_privacy)?)
    }

    pub async fn change_privacy(
        &self,
        space: &SpaceId,
        change: PrivacyChange,
    ) -> discord_core::Result<()> {
        let space = *space;
        self.work(move |session| async move {
            let patch = session
                .state()
                .privacy_patch(space, change)
                .ok_or_else(unreadable_privacy)?;
            patch_proto(&session, patch).await
        })
        .await
    }
}
