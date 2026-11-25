use discord_core::{
    ChannelId, ImageUpload, Integration, IntegrationId, InviteRequest, SpaceId, SpaceInvite,
    Webhook, WebhookId,
};
use discord_translate as translate;
use discord_wire as wire;
use serde_json::{Map, Value, json};

use super::image;
use crate::client::Client;

impl Client {
    pub async fn space_invites(&self, space: &SpaceId) -> discord_core::Result<Vec<SpaceInvite>> {
        let space = space.to_string();
        self.work(move |session| async move {
            let invites: Vec<wire::Invite> = session
                .rest()
                .get(format!("/guilds/{space}/invites"))
                .parse()
                .await?;
            Ok(invites.iter().map(translate::invite).collect())
        })
        .await
    }

    pub async fn channel_invites(
        &self,
        channel: &ChannelId,
    ) -> discord_core::Result<Vec<SpaceInvite>> {
        let channel = channel.to_string();
        self.work(move |session| async move {
            let invites: Vec<wire::Invite> = session
                .rest()
                .get(format!("/channels/{channel}/invites"))
                .parse()
                .await?;
            Ok(invites.iter().map(translate::invite).collect())
        })
        .await
    }

    pub async fn create_invite(
        &self,
        channel: &ChannelId,
        request: InviteRequest,
    ) -> discord_core::Result<SpaceInvite> {
        let channel = channel.to_string();
        self.work(move |session| async move {
            let invite: wire::Invite = session
                .rest()
                .post(format!("/channels/{channel}/invites"))
                .context(&json!({ "location": "Guild Header" }))
                .json(json!({
                    "max_age": request.age.seconds(),
                    "max_uses": request.max_uses,
                    "temporary": request.temporary,
                    "flags": 0,
                    "target_type": null,
                    "target_user_id": null,
                    "target_application_id": null,
                }))
                .parse()
                .await?;
            Ok(translate::invite(&invite))
        })
        .await
    }

    pub async fn revoke_invite(&self, code: &str) -> discord_core::Result<()> {
        let code = code.to_owned();
        self.work(move |session| async move {
            session
                .rest()
                .delete(format!("/invites/{code}"))
                .done()
                .await
        })
        .await
    }

    pub async fn space_webhooks(&self, space: &SpaceId) -> discord_core::Result<Vec<Webhook>> {
        let space = space.to_string();
        self.work(move |session| async move {
            let hooks: Vec<wire::Webhook> = session
                .rest()
                .get(format!("/guilds/{space}/webhooks"))
                .parse()
                .await?;
            Ok(hooks.iter().map(translate::webhook).collect())
        })
        .await
    }

    pub async fn channel_webhooks(
        &self,
        channel: &ChannelId,
    ) -> discord_core::Result<Vec<Webhook>> {
        let channel = channel.to_string();
        self.work(move |session| async move {
            let hooks: Vec<wire::Webhook> = session
                .rest()
                .get(format!("/channels/{channel}/webhooks"))
                .parse()
                .await?;
            Ok(hooks.iter().map(translate::webhook).collect())
        })
        .await
    }

    pub async fn create_webhook(
        &self,
        channel: &ChannelId,
        name: &str,
    ) -> discord_core::Result<Webhook> {
        let (channel, name) = (channel.to_string(), name.to_owned());
        self.work(move |session| async move {
            let hook: wire::Webhook = session
                .rest()
                .post(format!("/channels/{channel}/webhooks"))
                .json(json!({ "name": name }))
                .parse()
                .await?;
            Ok(translate::webhook(&hook))
        })
        .await
    }

    pub async fn edit_webhook(
        &self,
        hook: &WebhookId,
        name: &str,
        channel: &ChannelId,
        avatar: Option<Option<ImageUpload>>,
    ) -> discord_core::Result<Webhook> {
        let hook = hook.to_string();
        let mut body = Map::new();
        body.insert("name".to_owned(), json!(name.trim()));
        body.insert("channel_id".to_owned(), json!(channel.to_string()));
        if let Some(avatar) = avatar {
            body.insert("avatar".to_owned(), image(avatar.as_ref()));
        }
        self.work(move |session| async move {
            let hook: wire::Webhook = session
                .rest()
                .patch(format!("/webhooks/{hook}"))
                .json(Value::Object(body))
                .parse()
                .await?;
            Ok(translate::webhook(&hook))
        })
        .await
    }

    pub async fn delete_webhook(&self, hook: &WebhookId) -> discord_core::Result<()> {
        let hook = hook.to_string();
        self.work(move |session| async move {
            session
                .rest()
                .delete(format!("/webhooks/{hook}"))
                .done()
                .await
        })
        .await
    }

    pub async fn integrations(&self, space: &SpaceId) -> discord_core::Result<Vec<Integration>> {
        let space = space.to_string();
        self.work(move |session| async move {
            let integrations: Vec<wire::Integration> = session
                .rest()
                .get(format!("/guilds/{space}/integrations"))
                .query("include_applications", true)
                .parse()
                .await?;
            Ok(integrations.iter().map(translate::integration).collect())
        })
        .await
    }

    pub async fn remove_integration(
        &self,
        space: &SpaceId,
        integration: &IntegrationId,
    ) -> discord_core::Result<()> {
        let (space, integration) = (space.to_string(), integration.to_string());
        self.work(move |session| async move {
            session
                .rest()
                .delete(format!("/guilds/{space}/integrations/{integration}"))
                .done()
                .await
        })
        .await
    }
}
