use discord_core::Abilities;

use discord_wire::Overwrite;
use discord_wire::Role;

pub(crate) mod bits {
    use discord_core::Permissions;

    pub(crate) const ADMINISTRATOR: u64 = Permissions::ADMINISTRATOR.bits();
    pub(crate) const ADD_REACTIONS: u64 = Permissions::ADD_REACTIONS.bits();
    pub(crate) const VIEW_CHANNEL: u64 = Permissions::VIEW_CHANNEL.bits();
    pub(crate) const SEND_MESSAGES: u64 = Permissions::SEND_MESSAGES.bits();
    pub(crate) const STREAM: u64 = Permissions::STREAM.bits();
    pub(crate) const MANAGE_MESSAGES: u64 = Permissions::MANAGE_MESSAGES.bits();
    pub(crate) const EMBED_LINKS: u64 = Permissions::EMBED_LINKS.bits();
    pub(crate) const ATTACH_FILES: u64 = Permissions::ATTACH_FILES.bits();
    pub(crate) const READ_MESSAGE_HISTORY: u64 = Permissions::READ_MESSAGE_HISTORY.bits();
    pub(crate) const MENTION_EVERYONE: u64 = Permissions::MENTION_EVERYONE.bits();
    pub(crate) const USE_EXTERNAL_EMOJIS: u64 = Permissions::USE_EXTERNAL_EMOJIS.bits();
    pub(crate) const CONNECT: u64 = Permissions::CONNECT.bits();
    pub(crate) const SPEAK: u64 = Permissions::SPEAK.bits();
    pub(crate) const MUTE_MEMBERS: u64 = Permissions::MUTE_MEMBERS.bits();
    pub(crate) const USE_VAD: u64 = Permissions::USE_VAD.bits();
    pub(crate) const MANAGE_THREADS: u64 = Permissions::MANAGE_THREADS.bits();
    pub(crate) const CREATE_PUBLIC_THREADS: u64 = Permissions::CREATE_PUBLIC_THREADS.bits();
    pub(crate) const USE_EXTERNAL_STICKERS: u64 = Permissions::USE_EXTERNAL_STICKERS.bits();
    pub(crate) const SEND_MESSAGES_IN_THREADS: u64 = Permissions::SEND_MESSAGES_IN_THREADS.bits();
    pub(crate) const PIN_MESSAGES: u64 = Permissions::PIN_MESSAGES.bits();
    pub(crate) const ALL: u64 = Permissions::ALL.bits();
}

pub(crate) struct Viewer<'a> {
    pub(crate) user: &'a str,
    pub(crate) guild: &'a str,
    pub(crate) owner: Option<&'a str>,
    pub(crate) roles: &'a [String],
    pub(crate) timed_out: bool,
}

pub(crate) fn base(viewer: &Viewer<'_>, roles: &[Role]) -> u64 {
    if viewer.owner == Some(viewer.user) {
        return bits::ALL;
    }
    let mut permissions = roles
        .iter()
        .filter(|role| role.id == viewer.guild || viewer.roles.contains(&role.id))
        .fold(0, |combined, role| combined | role.permissions);
    if permissions & bits::ADMINISTRATOR != 0 {
        return bits::ALL;
    }
    if viewer.timed_out {
        permissions &= bits::VIEW_CHANNEL | bits::READ_MESSAGE_HISTORY;
    }
    permissions
}

pub(crate) fn in_channel(viewer: &Viewer<'_>, base: u64, overwrites: &[Overwrite]) -> u64 {
    if base == bits::ALL {
        return bits::ALL;
    }
    let mut permissions = base;
    if let Some(everyone) = overwrites
        .iter()
        .find(|overwrite| !overwrite.is_member() && overwrite.id == viewer.guild)
    {
        permissions = (permissions & !everyone.deny) | everyone.allow;
    }
    let (allow, deny) = overwrites
        .iter()
        .filter(|overwrite| !overwrite.is_member() && viewer.roles.contains(&overwrite.id))
        .fold((0, 0), |(allow, deny), overwrite| {
            (allow | overwrite.allow, deny | overwrite.deny)
        });
    permissions = (permissions & !deny) | allow;
    if let Some(member) = overwrites
        .iter()
        .find(|overwrite| overwrite.is_member() && overwrite.id == viewer.user)
    {
        permissions = (permissions & !member.deny) | member.allow;
    }
    if viewer.timed_out {
        permissions &= bits::VIEW_CHANNEL | bits::READ_MESSAGE_HISTORY;
    }
    permissions
}

pub(crate) fn abilities(permissions: u64, thread: Option<bool>) -> Abilities {
    let has = |bit: u64| permissions & bit != 0;
    if !has(bits::VIEW_CHANNEL) {
        return Abilities::NONE;
    }
    let can_send = match thread {
        Some(locked) => {
            has(bits::SEND_MESSAGES_IN_THREADS) && (!locked || has(bits::MANAGE_THREADS))
        }
        None => has(bits::SEND_MESSAGES),
    };
    let table = [
        (true, Abilities::VIEW),
        (has(bits::READ_MESSAGE_HISTORY), Abilities::READ_HISTORY),
        (can_send, Abilities::SEND),
        (
            has(bits::SEND_MESSAGES_IN_THREADS),
            Abilities::SEND_IN_THREADS,
        ),
        (can_send && has(bits::ATTACH_FILES), Abilities::ATTACH),
        (can_send && has(bits::EMBED_LINKS), Abilities::EMBED_LINKS),
        (has(bits::ADD_REACTIONS), Abilities::REACT),
        (has(bits::USE_EXTERNAL_EMOJIS), Abilities::EXTERNAL_EMOJI),
        (
            can_send && has(bits::MENTION_EVERYONE),
            Abilities::MENTION_EVERYONE,
        ),
        (has(bits::MANAGE_MESSAGES), Abilities::MANAGE_MESSAGES),
        (
            has(bits::PIN_MESSAGES) || has(bits::MANAGE_MESSAGES),
            Abilities::PIN,
        ),
        (has(bits::CREATE_PUBLIC_THREADS), Abilities::CREATE_THREADS),
        (has(bits::CONNECT), Abilities::CONNECT),
        (has(bits::SPEAK), Abilities::SPEAK),
        (has(bits::USE_VAD), Abilities::VOICE_ACTIVITY),
        (has(bits::STREAM), Abilities::STREAM),
        (has(bits::MUTE_MEMBERS), Abilities::MUTE_MEMBERS),
        (
            can_send && has(bits::USE_EXTERNAL_STICKERS),
            Abilities::STICKERS,
        ),
    ];
    table
        .into_iter()
        .filter(|(allowed, _)| *allowed)
        .fold(Abilities::NONE, |abilities, (_, ability)| {
            abilities | ability
        })
}

pub(crate) fn private() -> Abilities {
    [
        Abilities::VIEW,
        Abilities::READ_HISTORY,
        Abilities::SEND,
        Abilities::ATTACH,
        Abilities::EMBED_LINKS,
        Abilities::REACT,
        Abilities::EXTERNAL_EMOJI,
        Abilities::PIN,
        Abilities::CONNECT,
        Abilities::SPEAK,
        Abilities::VOICE_ACTIVITY,
        Abilities::STREAM,
        Abilities::STICKERS,
    ]
    .into_iter()
    .fold(Abilities::NONE, |abilities, ability| abilities | ability)
}
