#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Permissions(u64);

impl Permissions {
    pub const CREATE_INSTANT_INVITE: Permissions = Permissions(1);
    pub const KICK_MEMBERS: Permissions = Permissions(1 << 1);
    pub const BAN_MEMBERS: Permissions = Permissions(1 << 2);
    pub const ADMINISTRATOR: Permissions = Permissions(1 << 3);
    pub const MANAGE_CHANNELS: Permissions = Permissions(1 << 4);
    pub const MANAGE_GUILD: Permissions = Permissions(1 << 5);
    pub const ADD_REACTIONS: Permissions = Permissions(1 << 6);
    pub const VIEW_AUDIT_LOG: Permissions = Permissions(1 << 7);
    pub const PRIORITY_SPEAKER: Permissions = Permissions(1 << 8);
    pub const STREAM: Permissions = Permissions(1 << 9);
    pub const VIEW_CHANNEL: Permissions = Permissions(1 << 10);
    pub const SEND_MESSAGES: Permissions = Permissions(1 << 11);
    pub const SEND_TTS_MESSAGES: Permissions = Permissions(1 << 12);
    pub const MANAGE_MESSAGES: Permissions = Permissions(1 << 13);
    pub const EMBED_LINKS: Permissions = Permissions(1 << 14);
    pub const ATTACH_FILES: Permissions = Permissions(1 << 15);
    pub const READ_MESSAGE_HISTORY: Permissions = Permissions(1 << 16);
    pub const MENTION_EVERYONE: Permissions = Permissions(1 << 17);
    pub const USE_EXTERNAL_EMOJIS: Permissions = Permissions(1 << 18);
    pub const VIEW_GUILD_INSIGHTS: Permissions = Permissions(1 << 19);
    pub const CONNECT: Permissions = Permissions(1 << 20);
    pub const SPEAK: Permissions = Permissions(1 << 21);
    pub const MUTE_MEMBERS: Permissions = Permissions(1 << 22);
    pub const DEAFEN_MEMBERS: Permissions = Permissions(1 << 23);
    pub const MOVE_MEMBERS: Permissions = Permissions(1 << 24);
    pub const USE_VAD: Permissions = Permissions(1 << 25);
    pub const CHANGE_NICKNAME: Permissions = Permissions(1 << 26);
    pub const MANAGE_NICKNAMES: Permissions = Permissions(1 << 27);
    pub const MANAGE_ROLES: Permissions = Permissions(1 << 28);
    pub const MANAGE_WEBHOOKS: Permissions = Permissions(1 << 29);
    pub const MANAGE_GUILD_EXPRESSIONS: Permissions = Permissions(1 << 30);
    pub const USE_APPLICATION_COMMANDS: Permissions = Permissions(1 << 31);
    pub const REQUEST_TO_SPEAK: Permissions = Permissions(1 << 32);
    pub const MANAGE_EVENTS: Permissions = Permissions(1 << 33);
    pub const MANAGE_THREADS: Permissions = Permissions(1 << 34);
    pub const CREATE_PUBLIC_THREADS: Permissions = Permissions(1 << 35);
    pub const CREATE_PRIVATE_THREADS: Permissions = Permissions(1 << 36);
    pub const USE_EXTERNAL_STICKERS: Permissions = Permissions(1 << 37);
    pub const SEND_MESSAGES_IN_THREADS: Permissions = Permissions(1 << 38);
    pub const USE_EMBEDDED_ACTIVITIES: Permissions = Permissions(1 << 39);
    pub const MODERATE_MEMBERS: Permissions = Permissions(1 << 40);
    pub const VIEW_CREATOR_MONETIZATION_ANALYTICS: Permissions = Permissions(1 << 41);
    pub const USE_SOUNDBOARD: Permissions = Permissions(1 << 42);
    pub const CREATE_GUILD_EXPRESSIONS: Permissions = Permissions(1 << 43);
    pub const CREATE_EVENTS: Permissions = Permissions(1 << 44);
    pub const USE_EXTERNAL_SOUNDS: Permissions = Permissions(1 << 45);
    pub const SEND_VOICE_MESSAGES: Permissions = Permissions(1 << 46);
    pub const SEND_POLLS: Permissions = Permissions(1 << 49);
    pub const USE_EXTERNAL_APPS: Permissions = Permissions(1 << 50);
    pub const PIN_MESSAGES: Permissions = Permissions(1 << 51);
    pub const BYPASS_SLOWMODE: Permissions = Permissions(1 << 52);

    pub const NONE: Permissions = Permissions(0);
    pub const ALL: Permissions = Permissions(u64::MAX);

    #[must_use]
    pub const fn from_bits(bits: u64) -> Permissions {
        Permissions(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn contains(self, other: Permissions) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn intersects(self, other: Permissions) -> bool {
        self.0 & other.0 != 0
    }

    #[must_use]
    pub const fn union(self, other: Permissions) -> Permissions {
        Permissions(self.0 | other.0)
    }

    #[must_use]
    pub const fn without(self, other: Permissions) -> Permissions {
        Permissions(self.0 & !other.0)
    }

    #[must_use]
    pub const fn with(self, other: Permissions, on: bool) -> Permissions {
        match on {
            true => self.union(other),
            false => self.without(other),
        }
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for Permissions {
    type Output = Permissions;

    fn bitor(self, other: Permissions) -> Permissions {
        self.union(other)
    }
}

impl std::ops::BitOrAssign for Permissions {
    fn bitor_assign(&mut self, other: Permissions) {
        *self = self.union(other);
    }
}

impl std::fmt::Debug for Permissions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Permissions({:#x})", self.0)
    }
}
