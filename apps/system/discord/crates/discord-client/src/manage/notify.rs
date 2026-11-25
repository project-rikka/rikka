use discord_core::{ChannelId, Highlights, MuteSpan, NotifyChange, NotifyLevel, SpaceId};
use discord_state::Moment;
use discord_translate as translate;
use discord_wire::{notification_level, notify_highlights};
use serde_json::{Value, json};

use super::iso;
use crate::client::Client;
use crate::error::Result;
use crate::session::Session;

const FOREVER: i64 = -1;

fn mute_body(span: Option<MuteSpan>) -> Result<Value> {
    let Some(span) = span else {
        return Ok(json!({ "muted": false, "mute_config": null }));
    };
    let end = span
        .seconds()
        .map(|seconds| Moment::now().unix_ms + i64::from(seconds) * 1000);
    Ok(json!({
        "muted": true,
        "mute_config": {
            "selected_time_window": span.seconds().map_or(FOREVER, i64::from),
            "end_time": iso(end)?,
        },
    }))
}

fn change_body(change: NotifyChange) -> Value {
    match change {
        NotifyChange::Level(level) => {
            json!({ "message_notifications": translate::notify_code(level) })
        }
        NotifyChange::SuppressEveryone(on) => json!({ "suppress_everyone": on }),
        NotifyChange::SuppressRoles(on) => json!({ "suppress_roles": on }),
        NotifyChange::Highlights(highlights) => json!({
            "notify_highlights": match highlights {
                Highlights::Default => notify_highlights::DEFAULT,
                Highlights::Off => notify_highlights::DISABLED,
                Highlights::On => notify_highlights::ENABLED,
            }
        }),
        NotifyChange::MobilePush(on) => json!({ "mobile_push": on }),
        NotifyChange::HideMutedChannels(on) => json!({ "hide_muted_channels": on }),
        NotifyChange::MuteEvents(on) => json!({ "mute_scheduled_events": on }),
    }
}

async fn patch_settings(session: &Session, space: &str, body: Value) -> Result<()> {
    let bytes = session
        .rest()
        .patch(format!("/users/@me/guilds/{space}/settings"))
        .json(body)
        .send()
        .await?;
    session.absorb("USER_GUILD_SETTINGS_UPDATE", &bytes)
}

impl Client {
    pub async fn mute_space(
        &self,
        space: &SpaceId,
        span: Option<MuteSpan>,
    ) -> discord_core::Result<()> {
        let space = space.to_string();
        self.work(
            move |session| async move { patch_settings(&session, &space, mute_body(span)?).await },
        )
        .await
    }

    pub async fn change_space_notify(
        &self,
        space: &SpaceId,
        change: NotifyChange,
    ) -> discord_core::Result<()> {
        let space = space.to_string();
        self.work(move |session| async move {
            patch_settings(&session, &space, change_body(change)).await
        })
        .await
    }

    pub async fn mute_channel(
        &self,
        space: &SpaceId,
        channel: &ChannelId,
        span: Option<MuteSpan>,
    ) -> discord_core::Result<()> {
        let (space, channel) = (space.to_string(), channel.to_string());
        self.work(move |session| async move {
            let body = json!({ "channel_overrides": { channel: mute_body(span)? } });
            patch_settings(&session, &space, body).await
        })
        .await
    }

    pub async fn set_channel_level(
        &self,
        space: &SpaceId,
        channel: &ChannelId,
        level: Option<NotifyLevel>,
    ) -> discord_core::Result<()> {
        let (space, channel) = (space.to_string(), channel.to_string());
        let code = level.map_or(notification_level::INHERIT, translate::notify_code);
        self.work(move |session| async move {
            let body = json!({
                "channel_overrides": { channel: { "message_notifications": code } }
            });
            patch_settings(&session, &space, body).await
        })
        .await
    }
}
