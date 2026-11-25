use discord_core::{ChannelKind, Permissions};
use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Group {
    General,
    Membership,
    Text,
    Voice,
    Apps,
    Stage,
    Events,
    Advanced,
}

impl Group {
    pub(crate) const ALL: [Group; 8] = [
        Group::General,
        Group::Membership,
        Group::Text,
        Group::Voice,
        Group::Apps,
        Group::Stage,
        Group::Events,
        Group::Advanced,
    ];

    pub(crate) fn title(self) -> Label {
        match self {
            Group::General => tr!("discord.permissions.group-general"),
            Group::Membership => tr!("discord.permissions.group-membership"),
            Group::Text => tr!("discord.permissions.group-text"),
            Group::Voice => tr!("discord.permissions.group-voice"),
            Group::Apps => tr!("discord.permissions.group-apps"),
            Group::Stage => tr!("discord.permissions.group-stage"),
            Group::Events => tr!("discord.permissions.group-events"),
            Group::Advanced => tr!("discord.permissions.group-advanced"),
        }
    }

    pub(crate) fn entries(self, scope: Scope) -> Vec<Permissions> {
        let general_space = [
            Permissions::VIEW_CHANNEL,
            Permissions::MANAGE_CHANNELS,
            Permissions::MANAGE_ROLES,
            Permissions::CREATE_GUILD_EXPRESSIONS,
            Permissions::MANAGE_GUILD_EXPRESSIONS,
            Permissions::VIEW_AUDIT_LOG,
            Permissions::VIEW_GUILD_INSIGHTS,
            Permissions::MANAGE_WEBHOOKS,
            Permissions::MANAGE_GUILD,
        ];
        let general_channel = [
            Permissions::VIEW_CHANNEL,
            Permissions::MANAGE_CHANNELS,
            Permissions::MANAGE_ROLES,
            Permissions::MANAGE_WEBHOOKS,
        ];
        match (self, scope) {
            (Group::General, Scope::Space) => general_space.to_vec(),
            (Group::General, _) => general_channel.to_vec(),
            (Group::Membership, Scope::Space) => vec![
                Permissions::CREATE_INSTANT_INVITE,
                Permissions::CHANGE_NICKNAME,
                Permissions::MANAGE_NICKNAMES,
                Permissions::KICK_MEMBERS,
                Permissions::BAN_MEMBERS,
                Permissions::MODERATE_MEMBERS,
            ],
            (Group::Membership, _) => vec![Permissions::CREATE_INSTANT_INVITE],
            (Group::Text, Scope::Voice) => vec![
                Permissions::SEND_MESSAGES,
                Permissions::EMBED_LINKS,
                Permissions::ATTACH_FILES,
                Permissions::ADD_REACTIONS,
                Permissions::USE_EXTERNAL_EMOJIS,
                Permissions::USE_EXTERNAL_STICKERS,
                Permissions::MENTION_EVERYONE,
                Permissions::MANAGE_MESSAGES,
                Permissions::READ_MESSAGE_HISTORY,
                Permissions::SEND_TTS_MESSAGES,
                Permissions::SEND_VOICE_MESSAGES,
                Permissions::SEND_POLLS,
            ],
            (Group::Text, _) => vec![
                Permissions::SEND_MESSAGES,
                Permissions::SEND_MESSAGES_IN_THREADS,
                Permissions::CREATE_PUBLIC_THREADS,
                Permissions::CREATE_PRIVATE_THREADS,
                Permissions::EMBED_LINKS,
                Permissions::ATTACH_FILES,
                Permissions::ADD_REACTIONS,
                Permissions::USE_EXTERNAL_EMOJIS,
                Permissions::USE_EXTERNAL_STICKERS,
                Permissions::MENTION_EVERYONE,
                Permissions::MANAGE_MESSAGES,
                Permissions::PIN_MESSAGES,
                Permissions::BYPASS_SLOWMODE,
                Permissions::MANAGE_THREADS,
                Permissions::READ_MESSAGE_HISTORY,
                Permissions::SEND_TTS_MESSAGES,
                Permissions::SEND_VOICE_MESSAGES,
                Permissions::SEND_POLLS,
            ],
            (Group::Voice, Scope::Text) => Vec::new(),
            (Group::Voice, _) => vec![
                Permissions::CONNECT,
                Permissions::SPEAK,
                Permissions::STREAM,
                Permissions::USE_SOUNDBOARD,
                Permissions::USE_EXTERNAL_SOUNDS,
                Permissions::USE_VAD,
                Permissions::PRIORITY_SPEAKER,
                Permissions::MUTE_MEMBERS,
                Permissions::DEAFEN_MEMBERS,
                Permissions::MOVE_MEMBERS,
            ],
            (Group::Apps, _) => vec![
                Permissions::USE_APPLICATION_COMMANDS,
                Permissions::USE_EMBEDDED_ACTIVITIES,
                Permissions::USE_EXTERNAL_APPS,
            ],
            (Group::Stage, Scope::Text) => Vec::new(),
            (Group::Stage, _) => vec![Permissions::REQUEST_TO_SPEAK],
            (Group::Events, Scope::Text) => Vec::new(),
            (Group::Events, _) => vec![Permissions::CREATE_EVENTS, Permissions::MANAGE_EVENTS],
            (Group::Advanced, Scope::Space) => vec![Permissions::ADMINISTRATOR],
            (Group::Advanced, _) => Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Scope {
    Space,
    Text,
    Voice,
    Category,
}

impl Scope {
    pub(crate) fn of(kind: ChannelKind) -> Scope {
        match kind {
            ChannelKind::Voice | ChannelKind::Stage => Scope::Voice,
            ChannelKind::Category => Scope::Category,
            _ => Scope::Text,
        }
    }
}

pub(crate) fn label(permission: Permissions) -> Label {
    match permission {
        Permissions::CREATE_INSTANT_INVITE => tr!("discord.permissions.create-invite"),
        Permissions::KICK_MEMBERS => tr!("discord.permissions.kick-members"),
        Permissions::BAN_MEMBERS => tr!("discord.permissions.ban-members"),
        Permissions::ADMINISTRATOR => tr!("discord.permissions.administrator"),
        Permissions::MANAGE_CHANNELS => tr!("discord.permissions.manage-channels"),
        Permissions::MANAGE_GUILD => tr!("discord.permissions.manage-space"),
        Permissions::ADD_REACTIONS => tr!("discord.permissions.add-reactions"),
        Permissions::VIEW_AUDIT_LOG => tr!("discord.permissions.view-audit-log"),
        Permissions::PRIORITY_SPEAKER => tr!("discord.permissions.priority-speaker"),
        Permissions::STREAM => tr!("discord.permissions.video"),
        Permissions::VIEW_CHANNEL => tr!("discord.permissions.view-channels"),
        Permissions::SEND_MESSAGES => tr!("discord.permissions.send-messages"),
        Permissions::SEND_TTS_MESSAGES => tr!("discord.permissions.send-tts"),
        Permissions::MANAGE_MESSAGES => tr!("discord.permissions.manage-messages"),
        Permissions::EMBED_LINKS => tr!("discord.permissions.embed-links"),
        Permissions::ATTACH_FILES => tr!("discord.permissions.attach-files"),
        Permissions::READ_MESSAGE_HISTORY => tr!("discord.permissions.read-history"),
        Permissions::MENTION_EVERYONE => tr!("discord.permissions.mention-everyone"),
        Permissions::USE_EXTERNAL_EMOJIS => tr!("discord.permissions.external-emoji"),
        Permissions::VIEW_GUILD_INSIGHTS => tr!("discord.permissions.view-insights"),
        Permissions::CONNECT => tr!("discord.permissions.connect"),
        Permissions::SPEAK => tr!("discord.permissions.speak"),
        Permissions::MUTE_MEMBERS => tr!("discord.permissions.mute-members"),
        Permissions::DEAFEN_MEMBERS => tr!("discord.permissions.deafen-members"),
        Permissions::MOVE_MEMBERS => tr!("discord.permissions.move-members"),
        Permissions::USE_VAD => tr!("discord.permissions.voice-activity"),
        Permissions::CHANGE_NICKNAME => tr!("discord.permissions.change-nickname"),
        Permissions::MANAGE_NICKNAMES => tr!("discord.permissions.manage-nicknames"),
        Permissions::MANAGE_ROLES => tr!("discord.permissions.manage-roles"),
        Permissions::MANAGE_WEBHOOKS => tr!("discord.permissions.manage-webhooks"),
        Permissions::MANAGE_GUILD_EXPRESSIONS => tr!("discord.permissions.manage-expressions"),
        Permissions::USE_APPLICATION_COMMANDS => tr!("discord.permissions.app-commands"),
        Permissions::REQUEST_TO_SPEAK => tr!("discord.permissions.request-to-speak"),
        Permissions::MANAGE_EVENTS => tr!("discord.permissions.manage-events"),
        Permissions::MANAGE_THREADS => tr!("discord.permissions.manage-threads"),
        Permissions::CREATE_PUBLIC_THREADS => tr!("discord.permissions.public-threads"),
        Permissions::CREATE_PRIVATE_THREADS => tr!("discord.permissions.private-threads"),
        Permissions::USE_EXTERNAL_STICKERS => tr!("discord.permissions.external-stickers"),
        Permissions::SEND_MESSAGES_IN_THREADS => tr!("discord.permissions.send-in-threads"),
        Permissions::USE_EMBEDDED_ACTIVITIES => tr!("discord.permissions.activities"),
        Permissions::MODERATE_MEMBERS => tr!("discord.permissions.timeout-members"),
        Permissions::USE_SOUNDBOARD => tr!("discord.permissions.soundboard"),
        Permissions::CREATE_GUILD_EXPRESSIONS => tr!("discord.permissions.create-expressions"),
        Permissions::CREATE_EVENTS => tr!("discord.permissions.create-events"),
        Permissions::USE_EXTERNAL_SOUNDS => tr!("discord.permissions.external-sounds"),
        Permissions::SEND_VOICE_MESSAGES => tr!("discord.permissions.voice-messages"),
        Permissions::SEND_POLLS => tr!("discord.permissions.polls"),
        Permissions::USE_EXTERNAL_APPS => tr!("discord.permissions.external-apps"),
        Permissions::PIN_MESSAGES => tr!("discord.permissions.pin-messages"),
        Permissions::BYPASS_SLOWMODE => tr!("discord.permissions.bypass-slowmode"),
        _ => Label::from(format!("{permission:?}")),
    }
}
