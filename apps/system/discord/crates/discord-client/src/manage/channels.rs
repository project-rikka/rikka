use discord_core::{
    ChannelDraft, ChannelEdit, ChannelId, ChannelSettings, Overwrite, OverwriteTarget, Permissions,
    RoleId, SpaceId, VideoQuality,
};
use discord_translate as translate;
use discord_wire as wire;
use discord_wire::{overwrite_kind, snowflake, video_quality};
use serde_json::{Value, json};

use super::edits_body;
use crate::client::{Client, adopt_one};
use crate::error::Error;

fn edit_entry(edit: &ChannelEdit) -> (&'static str, Value) {
    match edit {
        ChannelEdit::Name(name) => ("name", json!(name.trim())),
        ChannelEdit::Topic(topic) => ("topic", json!(topic)),
        ChannelEdit::Nsfw(on) => ("nsfw", json!(on)),
        ChannelEdit::Slowmode(seconds) => ("rate_limit_per_user", json!(seconds)),
        ChannelEdit::Bitrate(bitrate) => ("bitrate", json!(bitrate)),
        ChannelEdit::UserLimit(limit) => ("user_limit", json!(limit)),
        ChannelEdit::Region(region) => ("rtc_region", json!(region)),
        ChannelEdit::VideoQuality(quality) => (
            "video_quality_mode",
            json!(match quality {
                VideoQuality::Auto => video_quality::AUTO,
                VideoQuality::Full => video_quality::FULL,
            }),
        ),
        ChannelEdit::ArchiveMinutes(minutes) => ("default_auto_archive_duration", json!(minutes)),
        ChannelEdit::ThreadSlowmode(seconds) => {
            ("default_thread_rate_limit_per_user", json!(seconds))
        }
    }
}

fn overwrite_body(overwrite: &Overwrite) -> Value {
    let kind = match overwrite.target {
        OverwriteTarget::Role(_) => overwrite_kind::ROLE,
        OverwriteTarget::Member(_) => overwrite_kind::MEMBER,
    };
    json!({
        "id": overwrite.target.raw().to_string(),
        "type": kind,
        "allow": overwrite.allow.bits().to_string(),
        "deny": overwrite.deny.bits().to_string(),
    })
}

impl Client {
    pub async fn channel_settings(
        &self,
        channel: &ChannelId,
    ) -> discord_core::Result<ChannelSettings> {
        let channel = channel.to_string();
        self.work(move |session| async move {
            let wire: wire::Channel = session
                .rest()
                .get(format!("/channels/{channel}"))
                .parse()
                .await?;
            translate::channel_settings(&wire)
                .ok_or_else(|| Error::Unreadable("Discord answered with an unknown channel".into()))
        })
        .await
    }

    pub async fn edit_channel(
        &self,
        channel: &ChannelId,
        edits: Vec<ChannelEdit>,
    ) -> discord_core::Result<ChannelSettings> {
        let channel = channel.to_string();
        let body = edits_body(&edits, edit_entry);
        self.work(move |session| async move {
            let wire: wire::Channel = session
                .rest()
                .patch(format!("/channels/{channel}"))
                .json(body)
                .parse()
                .await?;
            translate::channel_settings(&wire)
                .ok_or_else(|| Error::Unreadable("Discord answered with an unknown channel".into()))
        })
        .await
    }

    pub async fn set_overwrite(
        &self,
        channel: &ChannelId,
        overwrite: Overwrite,
    ) -> discord_core::Result<()> {
        let channel = channel.to_string();
        self.work(move |session| async move {
            session
                .rest()
                .put(format!(
                    "/channels/{channel}/permissions/{}",
                    overwrite.target.raw()
                ))
                .json(overwrite_body(&overwrite))
                .done()
                .await
        })
        .await
    }

    pub async fn remove_overwrite(
        &self,
        channel: &ChannelId,
        target: OverwriteTarget,
    ) -> discord_core::Result<()> {
        let channel = channel.to_string();
        self.work(move |session| async move {
            session
                .rest()
                .delete(format!("/channels/{channel}/permissions/{}", target.raw()))
                .done()
                .await
        })
        .await
    }

    pub async fn sync_permissions(
        &self,
        channel: &ChannelId,
        parent: &ChannelId,
    ) -> discord_core::Result<()> {
        let (channel, parent) = (channel.to_string(), parent.to_string());
        self.work(move |session| async move {
            session
                .rest()
                .patch(format!("/channels/{channel}"))
                .json(json!({ "parent_id": parent, "lock_permissions": true }))
                .done()
                .await
        })
        .await
    }

    pub async fn create_channel(
        &self,
        space: &SpaceId,
        draft: ChannelDraft,
    ) -> discord_core::Result<ChannelId> {
        let everyone = OverwriteTarget::Role(RoleId::new(space.get()));
        let overwrites: Vec<Value> = draft
            .private
            .then(|| {
                overwrite_body(&Overwrite {
                    deny: Permissions::VIEW_CHANNEL,
                    ..Overwrite::empty(everyone)
                })
            })
            .into_iter()
            .collect();
        let space = space.to_string();
        let body = json!({
            "name": draft.name.trim(),
            "type": translate::kind_code(draft.kind),
            "parent_id": draft.parent.map(|id| id.to_string()),
            "permission_overwrites": overwrites,
        });
        self.work(move |session| async move {
            let wire: wire::Channel = session
                .rest()
                .post(format!("/guilds/{space}/channels"))
                .json(body)
                .parse()
                .await?;
            adopt_one(&session, &wire)?;
            Ok(snowflake::id::<ChannelId>(wire.id.as_str()))
        })
        .await
    }

    pub async fn clone_channel(&self, channel: &ChannelId) -> discord_core::Result<ChannelId> {
        let channel = channel.to_string();
        self.work(move |session| async move {
            let source: wire::Channel = session
                .rest()
                .get(format!("/channels/{channel}"))
                .parse()
                .await?;
            let space = source
                .guild_id
                .clone()
                .ok_or_else(|| Error::Missing(format!("server of channel {channel}")))?;
            let overwrites: Vec<Value> = source
                .permission_overwrites
                .iter()
                .map(|overwrite| overwrite_body(&translate::overwrite(overwrite)))
                .collect();
            let wire: wire::Channel = session
                .rest()
                .post(format!("/guilds/{space}/channels"))
                .json(json!({
                    "name": source.name,
                    "type": source.kind,
                    "topic": source.topic,
                    "bitrate": source.bitrate,
                    "user_limit": source.user_limit,
                    "nsfw": source.nsfw,
                    "rate_limit_per_user": source.rate_limit_per_user,
                    "parent_id": source.parent_id,
                    "permission_overwrites": overwrites,
                }))
                .parse()
                .await?;
            adopt_one(&session, &wire)?;
            Ok(snowflake::id::<ChannelId>(wire.id.as_str()))
        })
        .await
    }

    pub async fn delete_channel(&self, channel: &ChannelId) -> discord_core::Result<()> {
        let channel = channel.to_string();
        self.work(move |session| async move {
            session
                .rest()
                .delete(format!("/channels/{channel}"))
                .done()
                .await
        })
        .await
    }
}
