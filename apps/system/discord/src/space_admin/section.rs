use discord_core::{Permissions, Space, SpaceId};
use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Group {
    Space,
    Apps,
    Moderation,
    Community,
    Monetization,
    People,
}

impl Group {
    pub(crate) fn title(self) -> Label {
        match self {
            Group::Space => tr!("discord.admin.group-space"),
            Group::Apps => tr!("discord.admin.group-apps"),
            Group::Moderation => tr!("discord.admin.group-moderation"),
            Group::Community => tr!("discord.admin.group-community"),
            Group::Monetization => tr!("discord.admin.group-monetization"),
            Group::People => tr!("discord.admin.group-people"),
        }
    }
}

pub(super) const BOOSTING: &str = "https://support.discord.com/hc/articles/360028038352";
const INSIGHTS: &str = "https://discord.com/developers/servers";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Section {
    #[default]
    Overview,
    Roles,
    Emoji,
    Stickers,
    Soundboard,
    Widget,
    Template,
    Vanity,
    Integrations,
    Safety,
    AutoMod,
    Community,
    Onboarding,
    Welcome,
    Boost,
    Subscriptions,
    Insights,
    Members,
    Invites,
    Bans,
    AuditLog,
    Delete,
}

impl Section {
    const ALL: [Section; 22] = [
        Section::Overview,
        Section::Roles,
        Section::Emoji,
        Section::Stickers,
        Section::Soundboard,
        Section::Widget,
        Section::Template,
        Section::Vanity,
        Section::Integrations,
        Section::Safety,
        Section::AutoMod,
        Section::Community,
        Section::Onboarding,
        Section::Welcome,
        Section::Boost,
        Section::Subscriptions,
        Section::Insights,
        Section::Members,
        Section::Invites,
        Section::Bans,
        Section::AuditLog,
        Section::Delete,
    ];

    fn allowed(self, space: &Space, owner: bool) -> bool {
        let any = |permissions: &[Permissions]| permissions.iter().any(|needed| space.can(*needed));
        match self {
            Section::Overview
            | Section::Widget
            | Section::Template
            | Section::Vanity
            | Section::Safety
            | Section::AutoMod
            | Section::Community
            | Section::Boost
            | Section::Subscriptions
            | Section::Invites => space.can(Permissions::MANAGE_GUILD),
            Section::Onboarding | Section::Welcome => {
                space.has(discord_core::COMMUNITY) && space.can(Permissions::MANAGE_GUILD)
            }
            Section::Roles => space.can(Permissions::MANAGE_ROLES),
            Section::Emoji | Section::Stickers | Section::Soundboard => any(&[
                Permissions::MANAGE_GUILD_EXPRESSIONS,
                Permissions::CREATE_GUILD_EXPRESSIONS,
            ]),
            Section::Integrations => {
                any(&[Permissions::MANAGE_WEBHOOKS, Permissions::MANAGE_GUILD])
            }
            Section::Insights => space.can(Permissions::VIEW_GUILD_INSIGHTS),
            Section::Members => any(&[
                Permissions::KICK_MEMBERS,
                Permissions::BAN_MEMBERS,
                Permissions::MANAGE_ROLES,
                Permissions::MODERATE_MEMBERS,
                Permissions::MANAGE_NICKNAMES,
                Permissions::MANAGE_GUILD,
            ]),
            Section::Bans => space.can(Permissions::BAN_MEMBERS),
            Section::AuditLog => space.can(Permissions::VIEW_AUDIT_LOG),
            Section::Delete => owner,
        }
    }

    pub(crate) fn available(space: &Space, owner: bool) -> Vec<Section> {
        Section::ALL
            .into_iter()
            .filter(|section| section.allowed(space, owner))
            .collect()
    }

    pub(crate) fn title(self) -> Label {
        match self {
            Section::Overview => tr!("discord.admin.overview"),
            Section::Roles => tr!("discord.admin.roles"),
            Section::Emoji => tr!("discord.admin.emoji"),
            Section::Stickers => tr!("discord.admin.stickers"),
            Section::Soundboard => tr!("discord.admin.soundboard"),
            Section::Widget => tr!("discord.admin.widget"),
            Section::Template => tr!("discord.admin.template"),
            Section::Vanity => tr!("discord.admin.vanity"),
            Section::Integrations => tr!("discord.admin.integrations"),
            Section::Safety => tr!("discord.admin.safety"),
            Section::AutoMod => tr!("discord.admin.automod"),
            Section::Community => tr!("discord.admin.community"),
            Section::Onboarding => tr!("discord.admin.onboarding"),
            Section::Welcome => tr!("discord.admin.welcome"),
            Section::Boost => tr!("discord.admin.boost"),
            Section::Subscriptions => tr!("discord.admin.subscriptions"),
            Section::Insights => tr!("discord.admin.insights"),
            Section::Members => tr!("discord.admin.members"),
            Section::Invites => tr!("discord.admin.invites"),
            Section::Bans => tr!("discord.admin.bans"),
            Section::AuditLog => tr!("discord.admin.audit-log"),
            Section::Delete => tr!("discord.admin.delete"),
        }
    }

    fn symbol(self) -> MaterialSymbol {
        match self {
            Section::Overview => MaterialSymbol::INFO,
            Section::Roles => MaterialSymbol::SHIELD_PERSON,
            Section::Emoji => MaterialSymbol::MOOD,
            Section::Stickers => MaterialSymbol::STICKY_NOTE_2,
            Section::Soundboard => MaterialSymbol::GRAPHIC_EQ,
            Section::Widget => MaterialSymbol::WIDGETS,
            Section::Template => MaterialSymbol::CONTENT_COPY,
            Section::Vanity => MaterialSymbol::LINK,
            Section::Integrations => MaterialSymbol::EXTENSION,
            Section::Safety => MaterialSymbol::SECURITY,
            Section::AutoMod => MaterialSymbol::SMART_TOY,
            Section::Community => MaterialSymbol::GROUPS,
            Section::Onboarding => MaterialSymbol::WAVING_HAND,
            Section::Welcome => MaterialSymbol::DOOR_OPEN,
            Section::Boost => MaterialSymbol::DIAMOND,
            Section::Subscriptions => MaterialSymbol::CARD_MEMBERSHIP,
            Section::Insights => MaterialSymbol::INSIGHTS,
            Section::Members => MaterialSymbol::GROUP,
            Section::Invites => MaterialSymbol::PERSON_ADD,
            Section::Bans => MaterialSymbol::BLOCK,
            Section::AuditLog => MaterialSymbol::HISTORY,
            Section::Delete => MaterialSymbol::DELETE_FOREVER,
        }
    }

    pub(crate) fn link(self, space: &SpaceId) -> Option<String> {
        match self {
            Section::Boost => Some(BOOSTING.to_owned()),
            Section::Subscriptions => Some(format!(
                "https://discord.com/channels/{space}/role-subscriptions"
            )),
            Section::Insights => Some(INSIGHTS.to_owned()),
            _ => None,
        }
    }

    pub(crate) fn is_link(self) -> bool {
        matches!(
            self,
            Section::Boost | Section::Subscriptions | Section::Insights
        )
    }

    pub(crate) fn group(self) -> Group {
        match self {
            Section::Overview
            | Section::Roles
            | Section::Emoji
            | Section::Stickers
            | Section::Soundboard
            | Section::Widget
            | Section::Template
            | Section::Vanity => Group::Space,
            Section::Integrations => Group::Apps,
            Section::Safety | Section::AutoMod => Group::Moderation,
            Section::Community | Section::Onboarding | Section::Welcome => Group::Community,
            Section::Boost | Section::Subscriptions | Section::Insights => Group::Monetization,
            Section::Members
            | Section::Invites
            | Section::Bans
            | Section::AuditLog
            | Section::Delete => Group::People,
        }
    }

    pub(crate) fn nav_item(self) -> NavItem {
        let symbol = match self.is_link() {
            true => MaterialSymbol::OPEN_IN_NEW,
            false => self.symbol(),
        };
        let item = nav_item(self.title()).icon(symbol);
        match self {
            Section::Delete => item.tint(palette().danger),
            _ => item,
        }
    }
}
