use discord_core::{Incidents, SpaceEdit, SpaceId, SpaceSettings, UserId};
use discord_translate as translate;
use discord_wire as wire;
use discord_wire::mfa_level;
use serde_json::{Value, json};

use super::{edits_body, image, iso};
use crate::client::Client;

fn edit_entry(edit: &SpaceEdit) -> (&'static str, Value) {
    let channel = |id: &Option<discord_core::ChannelId>| {
        id.map_or(Value::Null, |id| Value::String(id.to_string()))
    };
    match edit {
        SpaceEdit::Name(name) => ("name", json!(name.trim())),
        SpaceEdit::Icon(upload) => ("icon", image(upload.as_ref())),
        SpaceEdit::Banner(upload) => ("banner", image(upload.as_ref())),
        SpaceEdit::Splash(upload) => ("splash", image(upload.as_ref())),
        SpaceEdit::Description(text) => ("description", json!(text)),
        SpaceEdit::Verification(level) => ("verification_level", json!(level.code())),
        SpaceEdit::ContentFilter(filter) => ("explicit_content_filter", json!(filter.code())),
        SpaceEdit::DefaultNotifications(level) => (
            "default_message_notifications",
            json!(translate::notify_code(*level)),
        ),
        SpaceEdit::AfkChannel(id) => ("afk_channel_id", channel(id)),
        SpaceEdit::AfkTimeout(seconds) => ("afk_timeout", json!(seconds)),
        SpaceEdit::SystemChannel(id) => ("system_channel_id", channel(id)),
        SpaceEdit::SystemFlags(flags) => ("system_channel_flags", json!(flags.bits())),
        SpaceEdit::RulesChannel(id) => ("rules_channel_id", channel(id)),
        SpaceEdit::UpdatesChannel(id) => ("public_updates_channel_id", channel(id)),
        SpaceEdit::SafetyChannel(id) => ("safety_alerts_channel_id", channel(id)),
        SpaceEdit::Locale(locale) => ("preferred_locale", json!(locale)),
        SpaceEdit::ProgressBar(on) => ("premium_progress_bar_enabled", json!(on)),
        SpaceEdit::Features(features) => ("features", json!(features)),
    }
}

impl Client {
    pub async fn space_settings(&self, space: &SpaceId) -> discord_core::Result<SpaceSettings> {
        let space = space.to_string();
        self.work(move |session| async move {
            let guild: wire::Guild = session
                .rest()
                .get(format!("/guilds/{space}"))
                .query("with_counts", true)
                .parse()
                .await?;
            Ok(translate::space_settings(&guild))
        })
        .await
    }

    pub async fn edit_space(
        &self,
        space: &SpaceId,
        edits: Vec<SpaceEdit>,
    ) -> discord_core::Result<SpaceSettings> {
        let space = space.to_string();
        let body = edits_body(&edits, edit_entry);
        self.work(move |session| async move {
            let guild: wire::Guild = session
                .rest()
                .patch(format!("/guilds/{space}"))
                .json(body)
                .parse()
                .await?;
            Ok(translate::space_settings(&guild))
        })
        .await
    }

    pub async fn leave_space(&self, space: &SpaceId) -> discord_core::Result<()> {
        let space = space.to_string();
        self.work(move |session| async move {
            session
                .rest()
                .delete(format!("/users/@me/guilds/{space}"))
                .json(json!({ "lurking": false }))
                .done()
                .await
        })
        .await
    }

    pub async fn delete_space(&self, space: &SpaceId) -> discord_core::Result<()> {
        let space = space.to_string();
        self.work(move |session| async move {
            session
                .rest()
                .post(format!("/guilds/{space}/delete"))
                .json(json!({}))
                .done()
                .await
        })
        .await
    }

    pub async fn require_mfa(&self, space: &SpaceId, required: bool) -> discord_core::Result<()> {
        let space = space.to_string();
        let level = match required {
            true => mfa_level::ELEVATED,
            false => mfa_level::NONE,
        };
        self.work(move |session| async move {
            session
                .rest()
                .post(format!("/guilds/{space}/mfa"))
                .json(json!({ "level": level }))
                .done()
                .await
        })
        .await
    }

    pub async fn transfer_space(
        &self,
        space: &SpaceId,
        owner: &UserId,
    ) -> discord_core::Result<()> {
        let (space, owner) = (space.to_string(), owner.to_string());
        self.work(move |session| async move {
            session
                .rest()
                .patch(format!("/guilds/{space}"))
                .json(json!({ "owner_id": owner }))
                .done()
                .await
        })
        .await
    }

    pub async fn pause_incidents(
        &self,
        space: &SpaceId,
        invites_until: Option<i64>,
        directs_until: Option<i64>,
    ) -> discord_core::Result<Incidents> {
        let space = space.to_string();
        self.work(move |session| async move {
            let incidents: wire::Incidents = session
                .rest()
                .put(format!("/guilds/{space}/incident-actions"))
                .json(json!({
                    "invites_disabled_until": iso(invites_until)?,
                    "dms_disabled_until": iso(directs_until)?,
                }))
                .parse()
                .await?;
            Ok(translate::incidents(&incidents))
        })
        .await
    }
}
