#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NotifyLevel {
    #[default]
    All,
    Mentions,
    Nothing,
}

impl NotifyLevel {
    pub const ALL: [NotifyLevel; 3] = [
        NotifyLevel::All,
        NotifyLevel::Mentions,
        NotifyLevel::Nothing,
    ];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Highlights {
    #[default]
    Default,
    Off,
    On,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MuteSpan {
    Minutes15,
    Hour,
    Hours3,
    Hours8,
    Day,
    Forever,
}

impl MuteSpan {
    pub const ALL: [MuteSpan; 6] = [
        MuteSpan::Minutes15,
        MuteSpan::Hour,
        MuteSpan::Hours3,
        MuteSpan::Hours8,
        MuteSpan::Day,
        MuteSpan::Forever,
    ];

    #[must_use]
    pub const fn seconds(self) -> Option<u32> {
        match self {
            MuteSpan::Minutes15 => Some(15 * 60),
            MuteSpan::Hour => Some(60 * 60),
            MuteSpan::Hours3 => Some(3 * 60 * 60),
            MuteSpan::Hours8 => Some(8 * 60 * 60),
            MuteSpan::Day => Some(24 * 60 * 60),
            MuteSpan::Forever => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpaceNotify {
    pub level: NotifyLevel,
    pub inherited: bool,
    pub mute_ends: Option<i64>,
    pub suppress_everyone: bool,
    pub suppress_roles: bool,
    pub highlights: Highlights,
    pub mobile_push: bool,
    pub hide_muted_channels: bool,
    pub mute_events: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChannelNotify {
    pub level: Option<NotifyLevel>,
    pub resolved: NotifyLevel,
    pub mute_ends: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotifyChange {
    Level(NotifyLevel),
    SuppressEveryone(bool),
    SuppressRoles(bool),
    Highlights(Highlights),
    MobilePush(bool),
    HideMutedChannels(bool),
    MuteEvents(bool),
}
