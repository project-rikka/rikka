use discord_core::{Permissions, SpaceId, UserId};
use discord_voice::MAX_VOLUME;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::account::LinkedAccount;
use crate::chat::{Chat, now_ms};

const WIDTH: f32 = 220.0;
const ENTRY: f32 = 34.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MemberTarget {
    pub(crate) key: String,
    pub(crate) user: UserId,
    pub(crate) space: Option<SpaceId>,
    pub(crate) anchor: Anchor,
    pub(crate) over_profile: bool,
}

#[derive(Debug, State)]
pub(crate) struct MemberMenu {
    shown: Tracked<Option<MemberTarget>>,
}

impl MemberMenu {
    pub(crate) fn new() -> MemberMenu {
        MemberMenu {
            shown: Tracked::new(None),
        }
    }

    pub(crate) fn shown(&self) -> Option<&MemberTarget> {
        self.shown.as_ref()
    }

    pub(crate) fn show(&mut self, member: MemberTarget) {
        self.shown.set(Some(member));
    }

    pub(crate) fn close(&mut self) {
        self.shown.set(None);
    }
}

pub(crate) fn view(chat: &Chat) -> Option<Element> {
    let member = chat.member_menu().shown()?.clone();
    let linked = chat.linked(&member.key)?;
    let me = linked.is_me(&member.user);
    let seated = chat.calls().seat().filter(|seat| seat.key == member.key);
    let state = seated.and_then(|seat| {
        linked
            .voice_in(&seat.channel)
            .into_iter()
            .find(|state| state.user == member.user)
            .cloned()
    });
    let in_call = state.is_some();
    let stream = state
        .as_ref()
        .and_then(discord_core::VoiceState::stream)
        .filter(|_| !me);
    let watcher = member.key.clone();
    let volume = chat.user_volume(&member.key, &member.user);
    let (profiled, messaged, tuned, copied) = (
        member.clone(),
        member.clone(),
        member.clone(),
        member.user.to_string(),
    );
    Some(
        popover(
            column! {
                if !member.over_profile {
                    entry(MaterialSymbol::PERSON, tr!("discord.members.profile"))
                        .on_press(move |chat: &mut Chat| {
                            chat.open_profile(&profiled.user, profiled.space.as_ref(), profiled.anchor);
                        });
                }
                if !me {
                    entry(MaterialSymbol::CHAT_BUBBLE, tr!("discord.profile.message"))
                        .on_press(move |chat: &mut Chat| chat.message_user(&messaged.key, &messaged.user));
                }
                if let Some(key) = stream {
                    entry(MaterialSymbol::TV, tr!("discord.screen.watch"))
                        .on_press(move |chat: &mut Chat| {
                            chat.close_member_menu();
                            chat.watch_stream(&watcher, key);
                        });
                }
                if in_call && !me {
                    local_state(volume, move |shown: f32, set: Setter<f32>| {
                        let (tuned, volume_target) = (tuned.clone(), tuned.clone());
                        column! {
                            row! {
                                text(tr!("discord.members.volume")).caption().semibold().muted().grow(1.0);
                                text(format!("{}%", (shown * 100.0).round() as i64)).caption().muted();
                            }
                            .fill_width();
                            slider(shown)
                                .range(0.0..=MAX_VOLUME)
                                .step(0.05)
                                .on_change(move |chat: &mut Chat, volume: f32| {
                                    set(volume);
                                    chat.tune_user_volume(&tuned.key, &tuned.user, volume);
                                })
                                .on_commit(move |chat: &mut Chat, volume: f32| {
                                    chat.set_user_volume(&volume_target.key, &volume_target.user, volume);
                                })
                                .fill_width();
                        }
                        .gap(Space::TIGHT)
                        .padding(Inset::xy(Space::SNUG, Space::TIGHT))
                        .fill_width()
                    });
                }
                column_of(moderation(linked, &member)).gap(Space::HAIR).fill_width();
                entry(MaterialSymbol::CONTENT_COPY, tr!("discord.members.copy-id"))
                    .on_press(move |chat: &mut Chat| {
                        set_text_of(Selection::Clipboard, &copied);
                        chat.close_member_menu();
                    });
            }
            .gap(Space::HAIR)
            .width(WIDTH),
        )
        .on_dismiss(|menu: &mut MemberMenu| menu.close())
        .build()
        .anchored(member.anchor),
    )
}

fn moderation(linked: &LinkedAccount, member: &MemberTarget) -> Vec<Element> {
    let Some(space) = member.space else {
        return Vec::new();
    };
    let (Some(known), Some(me)) = (linked.space(&space), linked.me().map(|me| me.id)) else {
        return Vec::new();
    };
    let user = member.user;
    let mine = user == me;
    let owner = known.owned_by(&me);
    let outranks = linked.outranks(&space, &user);
    let can = |permission: Permissions| known.can(permission);
    let key = member.key.clone();
    let mut entries = Vec::new();
    let mut push = |symbol: MaterialSymbol,
                    label: Label,
                    danger: bool,
                    act: fn(&mut Chat, &str, &SpaceId, &UserId)| {
        let key = key.clone();
        let shown = match danger {
            true => tinted_entry(symbol, label, palette().danger, palette().danger),
            false => entry(symbol, label),
        };
        entries.push(
            shown
                .on_press(move |chat: &mut Chat| act(chat, &key, &space, &user))
                .any(),
        );
    };
    if (mine && can(Permissions::CHANGE_NICKNAME))
        || (!mine && outranks && can(Permissions::MANAGE_NICKNAMES))
    {
        push(
            MaterialSymbol::BADGE,
            tr!("discord.moderation.nickname"),
            false,
            Chat::ask_nickname,
        );
    }
    if can(Permissions::MANAGE_ROLES) {
        push(
            MaterialSymbol::SHIELD_PERSON,
            tr!("discord.moderation.roles"),
            false,
            Chat::ask_member_roles,
        );
    }
    if !mine && outranks && can(Permissions::MODERATE_MEMBERS) {
        let timed_out = linked
            .member(&space, &user)
            .and_then(|known| known.timeout_until)
            .is_some_and(|until| until > now_ms());
        match timed_out {
            true => push(
                MaterialSymbol::TIMER_OFF,
                tr!("discord.moderation.remove-timeout"),
                false,
                Chat::remove_timeout,
            ),
            false => push(
                MaterialSymbol::TIMER,
                tr!("discord.moderation.timeout"),
                true,
                Chat::ask_timeout,
            ),
        }
    }
    if !mine && outranks && can(Permissions::KICK_MEMBERS) {
        push(
            MaterialSymbol::PERSON_REMOVE,
            tr!("discord.moderation.kick"),
            true,
            Chat::ask_kick,
        );
    }
    if !mine && outranks && can(Permissions::BAN_MEMBERS) {
        push(
            MaterialSymbol::BLOCK,
            tr!("discord.moderation.ban"),
            true,
            Chat::ask_ban,
        );
    }
    if owner && !mine {
        push(
            MaterialSymbol::CROWN,
            tr!("discord.moderation.transfer"),
            true,
            Chat::ask_transfer,
        );
    }
    entries
}

fn entry(symbol: MaterialSymbol, label: Label) -> Container {
    tinted_entry(symbol, label, palette().text_muted, palette().text)
}

fn tinted_entry(symbol: MaterialSymbol, label: Label, mark: Color, ink: Color) -> Container {
    container(
        row! {
            icon(symbol).extent(Length::px(18.0)).color(mark);
            text(label).label().no_wrap().color(ink).grow(1.0);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .radius(Corner::Item)
    .focusless()
    .height(ENTRY)
    .fill_width()
}
