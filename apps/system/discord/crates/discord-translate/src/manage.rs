use discord_core::{
    AuditAction, AuditChange, AuditEntry, AuditEntryId, AutomodRule, Ban, ChannelId,
    ChannelSettings, ContentFilter, EmojiId, Incidents, Integration, IntegrationId, KeywordPreset,
    MemberHit, MfaChallenge, MfaMethod, NotifyLevel, Onboarding, OnboardingMode, Overwrite,
    OverwriteTarget, Permissions, Prompt, PromptOption, RoleId, RuleAction, RuleId, RuleTrigger,
    RuleTriggerData, SoundId, SpaceEmoji, SpaceId, SpaceInvite, SpaceSettings, SpaceSound,
    SpaceSticker, SystemFlags, Template, UserId, Vanity, VerificationLevel, VideoQuality, Webhook,
    WebhookId, WelcomeChannel, WelcomeScreen, Widget,
};

use super::{channel, guild, instant, user};
use crate::cdn;
use discord_wire as wire;
use discord_wire::snowflake;
use discord_wire::{
    automod_action, mfa_level, notification_level, onboarding_mode, overwrite_kind, prompt_kind,
    video_quality, webhook_kind,
};

#[must_use]
pub fn incidents(wire: &wire::Incidents) -> Incidents {
    Incidents {
        invites_paused_until: instant(wire.invites_disabled_until.as_deref()),
        directs_paused_until: instant(wire.dms_disabled_until.as_deref()),
    }
}

#[must_use]
pub fn notify_level(code: u8) -> Option<NotifyLevel> {
    match code {
        notification_level::ALL => Some(NotifyLevel::All),
        notification_level::MENTIONS => Some(NotifyLevel::Mentions),
        notification_level::NOTHING => Some(NotifyLevel::Nothing),
        _ => None,
    }
}

#[must_use]
pub fn notify_code(level: NotifyLevel) -> u8 {
    match level {
        NotifyLevel::All => notification_level::ALL,
        NotifyLevel::Mentions => notification_level::MENTIONS,
        NotifyLevel::Nothing => notification_level::NOTHING,
    }
}

fn ids<T: From<u64>>(raw: &[String]) -> Vec<T> {
    raw.iter().map(|id| snowflake::id::<T>(id)).collect()
}

fn channel_id(raw: Option<&str>) -> Option<ChannelId> {
    raw.map(snowflake::id::<ChannelId>)
}

pub fn space_settings(wire: &wire::Guild) -> SpaceSettings {
    let properties = wire.properties();
    let id = wire.id.as_str();
    SpaceSettings {
        id: snowflake::id::<SpaceId>(id),
        name: properties.name.clone().unwrap_or_default(),
        icon: properties
            .icon
            .as_deref()
            .map(|hash| cdn::guild_icon(id, hash)),
        banner: properties
            .banner
            .as_deref()
            .map(|hash| cdn::guild_banner(id, hash)),
        splash: wire
            .splash
            .as_deref()
            .map(|hash| cdn::guild_splash(id, hash)),
        description: wire.description.clone().filter(|text| !text.is_empty()),
        owner: properties.owner_id.as_deref().map(snowflake::id::<UserId>),
        verification: VerificationLevel::from_code(wire.verification_level),
        content_filter: ContentFilter::from_code(wire.explicit_content_filter),
        mfa_required: wire.mfa_level == mfa_level::ELEVATED,
        default_notifications: properties
            .default_message_notifications
            .and_then(notify_level)
            .unwrap_or_default(),
        afk_channel: channel_id(wire.afk_channel_id.as_deref()),
        afk_timeout: wire.afk_timeout,
        system_channel: channel_id(wire.system_channel_id.as_deref()),
        system_flags: SystemFlags::from_bits(wire.system_channel_flags),
        rules_channel: channel_id(wire.rules_channel_id.as_deref()),
        updates_channel: channel_id(wire.public_updates_channel_id.as_deref()),
        safety_channel: channel_id(wire.safety_alerts_channel_id.as_deref()),
        locale: wire
            .preferred_locale
            .clone()
            .unwrap_or_else(|| "en-US".to_owned()),
        features: wire.features.clone(),
        premium_tier: wire.premium_tier,
        boosts: wire.premium_subscription_count.unwrap_or(0),
        vanity: wire.vanity_url_code.clone(),
        progress_bar: wire.premium_progress_bar_enabled,
        members: wire.approximate_member_count,
        incidents: incidents(&wire.incidents_data.clone().unwrap_or_default()),
    }
}

#[must_use]
pub fn overwrite(wire: &wire::Overwrite) -> Overwrite {
    let target = match wire.kind {
        overwrite_kind::ROLE => OverwriteTarget::Role(snowflake::id::<RoleId>(wire.id.as_str())),
        _ => OverwriteTarget::Member(snowflake::id::<UserId>(wire.id.as_str())),
    };
    Overwrite {
        target,
        allow: Permissions::from_bits(wire.allow),
        deny: Permissions::from_bits(wire.deny),
    }
}

pub fn channel_settings(wire: &wire::Channel) -> Option<ChannelSettings> {
    Some(ChannelSettings {
        id: snowflake::id::<ChannelId>(wire.id.as_str()),
        space: snowflake::id::<SpaceId>(wire.guild_id.as_deref()?),
        kind: channel::kind(wire.kind)?,
        name: wire.name.clone().unwrap_or_default(),
        topic: wire.topic.clone().unwrap_or_default(),
        nsfw: wire.nsfw,
        slowmode: wire.rate_limit_per_user.unwrap_or(0),
        bitrate: wire.bitrate.unwrap_or(0),
        user_limit: wire.user_limit.unwrap_or(0),
        region: wire.rtc_region.clone(),
        video_quality: match wire.video_quality_mode.unwrap_or(video_quality::AUTO) {
            video_quality::AUTO => VideoQuality::Auto,
            _ => VideoQuality::Full,
        },
        parent: channel_id(wire.parent_id.as_deref()),
        overwrites: wire.permission_overwrites.iter().map(overwrite).collect(),
        archive_minutes: wire.default_auto_archive_duration.unwrap_or(4320),
        thread_slowmode: wire.default_thread_rate_limit_per_user.unwrap_or(0),
    })
}

#[must_use]
pub fn ban(wire: &wire::Ban) -> Ban {
    Ban {
        user: user(&wire.user),
        reason: wire.reason.clone().filter(|reason| !reason.is_empty()),
    }
}

pub fn audit_log(wire: &wire::AuditLog) -> Vec<AuditEntry> {
    wire.audit_log_entries
        .iter()
        .map(|entry| AuditEntry {
            id: snowflake::id::<AuditEntryId>(entry.id.as_str()),
            actor: entry
                .user_id
                .as_deref()
                .and_then(|actor| wire.users.iter().find(|known| known.id == actor).map(user)),
            target: entry.target_id.clone(),
            action: AuditAction(entry.action_type),
            changes: entry
                .changes
                .iter()
                .map(|change| AuditChange {
                    key: change.key.clone(),
                    old: change.before(),
                    new: change.after(),
                })
                .collect(),
            reason: entry.reason.clone().filter(|reason| !reason.is_empty()),
        })
        .collect()
}

pub fn invite(wire: &wire::Invite) -> SpaceInvite {
    SpaceInvite {
        code: wire.code.clone(),
        inviter: wire.inviter.as_ref().map(user),
        channel: wire
            .channel
            .as_ref()
            .map(|channel| snowflake::id::<ChannelId>(channel.id.as_str())),
        channel_name: wire
            .channel
            .as_ref()
            .and_then(|channel| channel.name.clone()),
        uses: wire.uses,
        max_uses: wire.max_uses,
        max_age: wire.max_age,
        temporary: wire.temporary,
        expires: instant(wire.expires_at.as_deref()),
    }
}

pub fn webhook(wire: &wire::Webhook) -> Webhook {
    Webhook {
        id: snowflake::id::<WebhookId>(wire.id.as_str()),
        name: wire.name.clone().unwrap_or_default(),
        avatar: Some(cdn::avatar(&wire.id, wire.avatar.as_deref(), None)),
        channel: channel_id(wire.channel_id.as_deref()),
        creator: wire.user.as_ref().map(user),
        url: wire
            .token
            .as_deref()
            .map(|token| format!("https://discord.com/api/webhooks/{}/{token}", wire.id)),
        incoming: wire.kind == webhook_kind::INCOMING,
    }
}

pub fn integration(wire: &wire::Integration) -> Integration {
    Integration {
        id: snowflake::id::<IntegrationId>(wire.id.as_str()),
        name: wire.name.clone(),
        kind: wire.kind.clone(),
        enabled: wire.enabled,
        account: wire.account.as_ref().map(|account| account.name.clone()),
        application: wire
            .application
            .as_ref()
            .map(|application| application.name.clone()),
        icon: wire.application.as_ref().and_then(|application| {
            application
                .icon
                .as_deref()
                .map(|hash| cdn::app_icon(&application.id, hash))
        }),
        creator: wire.user.as_ref().map(user),
    }
}

pub fn space_emoji(wire: &wire::Emoji) -> Option<SpaceEmoji> {
    let emoji = guild::custom_emoji(wire)?;
    Some(SpaceEmoji {
        id: snowflake::id::<EmojiId>(wire.id.as_deref()?),
        name: emoji.name.clone(),
        animated: wire.animated,
        picture: emoji.picture?,
        roles: ids(&wire.roles),
        creator: wire.user.as_ref().map(user),
        available: wire.available.unwrap_or(true),
    })
}

pub fn space_sticker(wire: &wire::Sticker) -> SpaceSticker {
    let sticker = guild::sticker(
        &wire.id,
        &wire.name,
        wire.format_type,
        wire.description.as_deref(),
    );
    SpaceSticker {
        id: sticker.id,
        name: sticker.name,
        description: sticker.description,
        tags: wire.tags.clone().unwrap_or_default(),
        format: sticker.format,
        picture: sticker
            .picture
            .unwrap_or_else(|| cdn::sticker(&wire.id, sticker.format)),
        creator: wire.user.as_ref().map(user),
        available: wire.available.unwrap_or(true),
    }
}

pub fn space_sound(wire: &wire::Sound) -> SpaceSound {
    let emoji = channel::emoji_of(wire.emoji_id.as_deref(), wire.emoji_name.as_deref());
    SpaceSound {
        id: snowflake::id::<SoundId>(wire.sound_id.as_str()),
        name: wire.name.clone(),
        volume: wire.volume,
        emoji,
        creator: wire.user.as_ref().map(user),
        available: wire.available.unwrap_or(true),
        url: cdn::sound(&wire.sound_id),
    }
}

pub fn automod_rule(wire: &wire::AutomodRule) -> Option<AutomodRule> {
    let trigger = RuleTrigger::from_code(wire.trigger_type)?;
    let metadata = &wire.trigger_metadata;
    Some(AutomodRule {
        id: Some(snowflake::id::<RuleId>(wire.id.as_str())),
        name: wire.name.clone(),
        trigger,
        data: RuleTriggerData {
            keywords: metadata.keyword_filter.clone(),
            patterns: metadata.regex_patterns.clone(),
            presets: metadata
                .presets
                .iter()
                .filter_map(|code| KeywordPreset::from_code(*code))
                .collect(),
            allowed: metadata.allow_list.clone(),
            mention_limit: metadata.mention_total_limit,
            raid_protection: metadata.mention_raid_protection_enabled,
        },
        actions: wire.actions.iter().filter_map(rule_action).collect(),
        enabled: wire.enabled,
        exempt_roles: ids(&wire.exempt_roles),
        exempt_channels: ids(&wire.exempt_channels),
    })
}

fn rule_action(wire: &wire::AutomodAction) -> Option<RuleAction> {
    let metadata = &wire.metadata;
    match wire.kind {
        automod_action::BLOCK => Some(RuleAction::Block {
            message: metadata
                .custom_message
                .clone()
                .filter(|message| !message.is_empty()),
        }),
        automod_action::ALERT => Some(RuleAction::Alert {
            channel: channel_id(metadata.channel_id.as_deref())?,
        }),
        automod_action::TIMEOUT => Some(RuleAction::Timeout {
            seconds: metadata.duration_seconds.unwrap_or(60),
        }),
        automod_action::BLOCK_INTERACTIONS => Some(RuleAction::BlockInteractions),
        _ => None,
    }
}

pub fn welcome_screen(wire: &wire::WelcomeScreen, enabled: bool) -> WelcomeScreen {
    WelcomeScreen {
        enabled,
        description: wire.description.clone().unwrap_or_default(),
        channels: wire
            .welcome_channels
            .iter()
            .map(|entry| WelcomeChannel {
                channel: snowflake::id::<ChannelId>(entry.channel_id.as_str()),
                description: entry.description.clone(),
                emoji: channel::emoji_of(entry.emoji_id.as_deref(), entry.emoji_name.as_deref()),
            })
            .collect(),
    }
}

pub fn onboarding(wire: &wire::Onboarding) -> Onboarding {
    Onboarding {
        enabled: wire.enabled,
        mode: match wire.mode {
            onboarding_mode::ADVANCED => OnboardingMode::Advanced,
            _ => OnboardingMode::Default,
        },
        default_channels: ids(&wire.default_channel_ids),
        prompts: wire
            .prompts
            .iter()
            .map(|prompt| Prompt {
                id: prompt.id.clone(),
                title: prompt.title.clone(),
                options: prompt
                    .options
                    .iter()
                    .map(|option| PromptOption {
                        id: option.id.clone(),
                        title: option.title.clone(),
                        description: option.description.clone().unwrap_or_default(),
                        emoji: option.emoji.as_ref().and_then(|emoji| match emoji.id {
                            Some(_) => guild::custom_emoji(emoji),
                            None => channel::emoji_of(None, emoji.name.as_deref()),
                        }),
                        channels: ids(&option.channel_ids),
                        roles: ids(&option.role_ids),
                    })
                    .collect(),
                single_select: prompt.single_select,
                required: prompt.required,
                in_onboarding: prompt.in_onboarding,
                dropdown: prompt.kind == prompt_kind::DROPDOWN,
            })
            .collect(),
    }
}

#[must_use]
pub fn widget(wire: &wire::Widget) -> Widget {
    Widget {
        enabled: wire.enabled,
        channel: channel_id(wire.channel_id.as_deref()),
    }
}

pub fn template(wire: &wire::Template) -> Template {
    Template {
        code: wire.code.clone(),
        name: wire.name.clone(),
        description: wire.description.clone().filter(|text| !text.is_empty()),
        uses: wire.usage_count,
        creator: wire.creator_id.as_deref().map(snowflake::id::<UserId>),
        updated: instant(wire.updated_at.as_deref()),
        dirty: wire.is_dirty,
    }
}

#[must_use]
pub fn vanity(wire: &wire::Vanity) -> Vanity {
    Vanity {
        code: wire.code.clone().filter(|code| !code.is_empty()),
        uses: wire.uses,
    }
}

pub fn member_hit(space: &str, wire: &wire::MemberSearchHit) -> Option<MemberHit> {
    Some(MemberHit {
        member: super::member(space, &wire.member)?,
        user: user(wire.member.user.as_ref()?),
        invite: wire.source_invite_code.clone(),
    })
}

pub fn mfa_challenge(wire: &wire::MfaTicket) -> MfaChallenge {
    MfaChallenge {
        ticket: wire.ticket.clone(),
        methods: wire
            .methods
            .iter()
            .filter_map(|offer| MfaMethod::named(&offer.kind))
            .collect(),
    }
}
