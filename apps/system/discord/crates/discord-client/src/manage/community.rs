use discord_core::{
    AutomodRule, Emoji, Onboarding, OnboardingMode, RuleAction, RuleId, SpaceId, Template, Vanity,
    WelcomeScreen, Widget,
};
use discord_translate as translate;
use discord_wire as wire;
use discord_wire::{automod_action, onboarding_mode, prompt_kind};
use serde_json::{Value, json};

use super::snowflakes;
use crate::client::Client;
use crate::error::Error;

fn rule_body(rule: &AutomodRule) -> Value {
    let actions: Vec<Value> = rule
        .actions
        .iter()
        .map(|action| match action {
            RuleAction::Block { message } => json!({
                "type": automod_action::BLOCK,
                "metadata": { "custom_message": message.as_deref().unwrap_or_default() },
            }),
            RuleAction::Alert { channel } => json!({
                "type": automod_action::ALERT,
                "metadata": { "channel_id": channel.to_string() },
            }),
            RuleAction::Timeout { seconds } => json!({
                "type": automod_action::TIMEOUT,
                "metadata": { "duration_seconds": seconds },
            }),
            RuleAction::BlockInteractions => {
                json!({ "type": automod_action::BLOCK_INTERACTIONS, "metadata": {} })
            }
        })
        .collect();
    let data = &rule.data;
    json!({
        "name": rule.name,
        "event_type": rule.trigger.event(),
        "trigger_type": rule.trigger.code(),
        "trigger_metadata": {
            "keyword_filter": data.keywords,
            "regex_patterns": data.patterns,
            "presets": data.presets.iter().map(|preset| preset.code()).collect::<Vec<_>>(),
            "allow_list": data.allowed,
            "mention_total_limit": data.mention_limit,
            "mention_raid_protection_enabled": data.raid_protection,
        },
        "actions": actions,
        "enabled": rule.enabled,
        "exempt_roles": snowflakes(&rule.exempt_roles),
        "exempt_channels": snowflakes(&rule.exempt_channels),
    })
}

fn onboarding_body(onboarding: &Onboarding) -> Value {
    let prompts: Vec<Value> = onboarding
        .prompts
        .iter()
        .map(|prompt| {
            let options: Vec<Value> = prompt
                .options
                .iter()
                .map(|option| {
                    json!({
                        "id": option.id,
                        "title": option.title,
                        "description": option.description,
                        "emoji_id": option.emoji.as_ref().and_then(|emoji| emoji.id).map(|id| id.to_string()),
                        "emoji_name": option.emoji.as_ref().map(|emoji| emoji.name.clone()),
                        "emoji_animated": option.emoji.as_ref().is_some_and(Emoji::is_animated),
                        "channel_ids": snowflakes(&option.channels),
                        "role_ids": snowflakes(&option.roles),
                    })
                })
                .collect();
            json!({
                "id": prompt.id,
                "type": match prompt.dropdown {
                    true => prompt_kind::DROPDOWN,
                    false => prompt_kind::MULTIPLE_CHOICE,
                },
                "title": prompt.title,
                "options": options,
                "single_select": prompt.single_select,
                "required": prompt.required,
                "in_onboarding": prompt.in_onboarding,
            })
        })
        .collect();
    json!({
        "prompts": prompts,
        "default_channel_ids": snowflakes(&onboarding.default_channels),
        "enabled": onboarding.enabled,
        "mode": match onboarding.mode {
            OnboardingMode::Default => onboarding_mode::DEFAULT,
            OnboardingMode::Advanced => onboarding_mode::ADVANCED,
        },
    })
}

impl Client {
    pub async fn automod_rules(&self, space: &SpaceId) -> discord_core::Result<Vec<AutomodRule>> {
        let space = space.to_string();
        self.work(move |session| async move {
            let rules: Vec<wire::AutomodRule> = session
                .rest()
                .get(format!("/guilds/{space}/auto-moderation/rules"))
                .parse()
                .await?;
            Ok(rules.iter().filter_map(translate::automod_rule).collect())
        })
        .await
    }

    pub async fn save_automod_rule(
        &self,
        space: &SpaceId,
        rule: &AutomodRule,
    ) -> discord_core::Result<AutomodRule> {
        let space = space.to_string();
        let body = rule_body(rule);
        let id = rule.id;
        self.work(move |session| async move {
            let request = match id {
                Some(id) => session
                    .rest()
                    .patch(format!("/guilds/{space}/auto-moderation/rules/{id}")),
                None => session
                    .rest()
                    .post(format!("/guilds/{space}/auto-moderation/rules")),
            };
            let saved: wire::AutomodRule = request.json(body).parse().await?;
            translate::automod_rule(&saved)
                .ok_or_else(|| Error::Unreadable("Discord answered with an unknown rule".into()))
        })
        .await
    }

    pub async fn delete_automod_rule(
        &self,
        space: &SpaceId,
        rule: &RuleId,
    ) -> discord_core::Result<()> {
        let (space, rule) = (space.to_string(), rule.to_string());
        self.work(move |session| async move {
            session
                .rest()
                .delete(format!("/guilds/{space}/auto-moderation/rules/{rule}"))
                .done()
                .await
        })
        .await
    }

    pub async fn welcome_screen(
        &self,
        space: &SpaceId,
        enabled: bool,
    ) -> discord_core::Result<WelcomeScreen> {
        let space = space.to_string();
        self.work(move |session| async move {
            let screen: wire::WelcomeScreen = session
                .rest()
                .get(format!("/guilds/{space}/welcome-screen"))
                .parse()
                .await?;
            Ok(translate::welcome_screen(&screen, enabled))
        })
        .await
    }

    pub async fn save_welcome_screen(
        &self,
        space: &SpaceId,
        screen: &WelcomeScreen,
    ) -> discord_core::Result<()> {
        let space = space.to_string();
        let channels: Vec<Value> = screen
            .channels
            .iter()
            .map(|entry| {
                json!({
                    "channel_id": entry.channel.to_string(),
                    "description": entry.description,
                    "emoji_id": entry.emoji.as_ref().and_then(|emoji| emoji.id).map(|id| id.to_string()),
                    "emoji_name": entry.emoji.as_ref().map(|emoji| emoji.name.clone()),
                })
            })
            .collect();
        let body = json!({
            "enabled": screen.enabled,
            "description": screen.description,
            "welcome_channels": channels,
        });
        self.work(move |session| async move {
            session
                .rest()
                .patch(format!("/guilds/{space}/welcome-screen"))
                .json(body)
                .done()
                .await
        })
        .await
    }

    pub async fn onboarding(&self, space: &SpaceId) -> discord_core::Result<Onboarding> {
        let space = space.to_string();
        self.work(move |session| async move {
            let onboarding: wire::Onboarding = session
                .rest()
                .get(format!("/guilds/{space}/onboarding"))
                .parse()
                .await?;
            Ok(translate::onboarding(&onboarding))
        })
        .await
    }

    pub async fn save_onboarding(
        &self,
        space: &SpaceId,
        onboarding: &Onboarding,
    ) -> discord_core::Result<Onboarding> {
        let space = space.to_string();
        let body = onboarding_body(onboarding);
        self.work(move |session| async move {
            let saved: wire::Onboarding = session
                .rest()
                .put(format!("/guilds/{space}/onboarding"))
                .json(body)
                .parse()
                .await?;
            Ok(translate::onboarding(&saved))
        })
        .await
    }

    pub async fn widget(&self, space: &SpaceId) -> discord_core::Result<Widget> {
        let space = space.to_string();
        self.work(move |session| async move {
            let widget: wire::Widget = session
                .rest()
                .get(format!("/guilds/{space}/widget"))
                .parse()
                .await?;
            Ok(translate::widget(&widget))
        })
        .await
    }

    pub async fn save_widget(&self, space: &SpaceId, widget: &Widget) -> discord_core::Result<()> {
        let space = space.to_string();
        let body = json!({
            "enabled": widget.enabled,
            "channel_id": widget.channel.map(|id| id.to_string()),
        });
        self.work(move |session| async move {
            session
                .rest()
                .patch(format!("/guilds/{space}/widget"))
                .json(body)
                .done()
                .await
        })
        .await
    }

    pub async fn templates(&self, space: &SpaceId) -> discord_core::Result<Vec<Template>> {
        let space = space.to_string();
        self.work(move |session| async move {
            let templates: Vec<wire::Template> = session
                .rest()
                .get(format!("/guilds/{space}/templates"))
                .parse()
                .await?;
            Ok(templates.iter().map(translate::template).collect())
        })
        .await
    }

    pub async fn create_template(
        &self,
        space: &SpaceId,
        name: &str,
        description: &str,
    ) -> discord_core::Result<Template> {
        let space = space.to_string();
        let body = json!({ "name": name.trim(), "description": description.trim() });
        self.work(move |session| async move {
            let template: wire::Template = session
                .rest()
                .post(format!("/guilds/{space}/templates"))
                .json(body)
                .parse()
                .await?;
            Ok(translate::template(&template))
        })
        .await
    }

    pub async fn sync_template(
        &self,
        space: &SpaceId,
        code: &str,
    ) -> discord_core::Result<Template> {
        let (space, code) = (space.to_string(), code.to_owned());
        self.work(move |session| async move {
            let template: wire::Template = session
                .rest()
                .put(format!("/guilds/{space}/templates/{code}"))
                .parse()
                .await?;
            Ok(translate::template(&template))
        })
        .await
    }

    pub async fn delete_template(&self, space: &SpaceId, code: &str) -> discord_core::Result<()> {
        let (space, code) = (space.to_string(), code.to_owned());
        self.work(move |session| async move {
            session
                .rest()
                .delete(format!("/guilds/{space}/templates/{code}"))
                .done()
                .await
        })
        .await
    }

    pub async fn vanity(&self, space: &SpaceId) -> discord_core::Result<Vanity> {
        let space = space.to_string();
        self.work(move |session| async move {
            let vanity: wire::Vanity = session
                .rest()
                .get(format!("/guilds/{space}/vanity-url"))
                .parse()
                .await?;
            Ok(translate::vanity(&vanity))
        })
        .await
    }

    pub async fn save_vanity(&self, space: &SpaceId, code: &str) -> discord_core::Result<()> {
        let (space, code) = (space.to_string(), code.trim().to_owned());
        self.work(move |session| async move {
            session
                .rest()
                .patch(format!("/guilds/{space}/vanity-url"))
                .json(json!({ "code": code }))
                .done()
                .await
        })
        .await
    }
}
