use discord_core::{
    Abilities, Channel, ChannelId, ChannelKind, Picture, SpaceId, StreamKey, UserId, VoiceState,
};
use rikka_app::prelude::*;
use rikka_ui::Handler;
use rikka_widgets::row;

use super::rows::{Look, ROW, kind_symbol, trailing_badge, unread_dot};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::voice::Calls;
use crate::widgets::{avatar, context_area, live_badge, mark_of};

const CATEGORY: f32 = 40.0;
const VOICER: f32 = 30.0;
const VOICER_AVATAR: f32 = 22.0;

#[derive(Clone, Debug)]
enum Line {
    Category {
        id: ChannelId,
        name: String,
        collapsed: bool,
        unread: bool,
    },
    Channel(ChannelLine),
    Voicer(VoicerLine),
}

#[derive(Clone, Debug)]
struct ChannelLine {
    id: ChannelId,
    kind: ChannelKind,
    name: String,
    thread: bool,
    look: Look,
    nsfw: bool,
    locked: bool,
    seats: Option<(usize, u32)>,
    joined: bool,
}

#[derive(Clone, Debug)]
struct VoicerLine {
    account: String,
    user: UserId,
    space: SpaceId,
    name: String,
    picture: Option<Picture>,
    silenced: bool,
    unhearing: bool,
    stream: Option<StreamKey>,
    joined: bool,
}

pub(crate) fn view(chat: &Chat, space: &SpaceId) -> Element {
    let Some(linked) = chat.current() else {
        return spacer().any();
    };
    let lines = lines(chat, linked, space);
    lazy_column(lines.len(), move |slot| line(&lines[slot.index]))
        .padding(Inset::xy(Space::SNUG, Space::SNUG))
        .shows(*space)
        .fill_width()
        .grow(1.0)
        .any()
}

fn lines(chat: &Chat, linked: &LinkedAccount, space: &SpaceId) -> Vec<Line> {
    let selected = linked.location().channel;
    let folded = chat.collapsed_categories();
    let mut lines = Vec::new();
    for section in linked.sections(space) {
        let collapsed = section
            .category
            .is_some_and(|category| folded.contains(&category.id));
        if let Some(category) = section.category {
            let unread = section
                .rows
                .iter()
                .any(|row| Look::resolve(linked, row.channel, None).bold());
            lines.push(Line::Category {
                id: category.id,
                name: category.name.to_uppercase(),
                collapsed,
                unread,
            });
        }
        for row in &section.rows {
            let chosen = selected.as_ref() == Some(&row.channel.id);
            let threads_chosen = row
                .threads
                .iter()
                .any(|thread| selected.as_ref() == Some(&thread.id));
            if collapsed && !chosen && !threads_chosen {
                continue;
            }
            let voicers = match row.channel.kind.is_voice() {
                true => linked.voice_in(&row.channel.id),
                false => Vec::new(),
            };
            let joined = chat.calls().is_seated_in(linked.key(), &row.channel.id);
            if !collapsed || chosen {
                let mut line =
                    channel_line(linked, row.channel, false, selected.as_ref(), voicers.len());
                line.joined = joined;
                line.look.selected |= joined;
                lines.push(Line::Channel(line));
            }
            for thread in &row.threads {
                if collapsed && selected.as_ref() != Some(&thread.id) {
                    continue;
                }
                lines.push(Line::Channel(channel_line(
                    linked,
                    thread,
                    true,
                    selected.as_ref(),
                    0,
                )));
            }
            lines.extend(voicers.into_iter().map(|state| {
                let mut line = voicer_line(linked, space, state);
                line.joined = joined;
                Line::Voicer(line)
            }));
        }
    }
    lines
}

impl Look {
    fn resolve(linked: &LinkedAccount, channel: &Channel, selected: Option<&ChannelId>) -> Look {
        let parent_muted = channel
            .parent
            .as_ref()
            .and_then(|parent| linked.channel(parent))
            .is_some_and(|parent| parent.muted);
        Look {
            selected: selected == Some(&channel.id),
            unseen: linked.unseen(&channel.id),
            muted: channel.muted || parent_muted,
        }
    }
}

fn channel_line(
    linked: &LinkedAccount,
    channel: &Channel,
    thread: bool,
    selected: Option<&ChannelId>,
    connected: usize,
) -> ChannelLine {
    let locked = match channel.kind {
        ChannelKind::Voice | ChannelKind::Stage => !channel.abilities.contains(Abilities::CONNECT),
        ChannelKind::Thread => channel.thread.as_ref().is_some_and(|thread| thread.locked),
        _ => false,
    };
    ChannelLine {
        id: channel.id,
        kind: channel.kind,
        name: channel.name.clone(),
        thread,
        look: Look::resolve(linked, channel, selected),
        nsfw: channel.nsfw,
        locked,
        seats: channel
            .user_limit
            .filter(|limit| *limit > 0 && channel.kind.is_voice())
            .map(|limit| (connected, limit)),
        joined: false,
    }
}

fn voicer_line(linked: &LinkedAccount, space: &SpaceId, state: &VoiceState) -> VoicerLine {
    let user = linked.user(&state.user);
    let name = linked
        .name_of(Some(space), &state.user)
        .unwrap_or_else(|| state.user.to_string());
    VoicerLine {
        account: linked.key().to_owned(),
        user: state.user,
        space: *space,
        name,
        picture: user.and_then(|user| user.avatar.clone()),
        silenced: state.silenced(),
        unhearing: state.unhearing(),
        stream: state.stream(),
        joined: false,
    }
}
fn line(line: &Line) -> Element {
    match line {
        Line::Category {
            id,
            name,
            collapsed,
            unread,
        } => category(id, name, *collapsed, *unread),
        Line::Channel(channel) => channel_row(channel),
        Line::Voicer(voicer) => voicer_row(voicer),
    }
}

fn category(id: &ChannelId, name: &str, collapsed: bool, unread: bool) -> Element {
    let toggled = *id;
    let ink = match unread && collapsed {
        true => palette().text,
        false => palette().text_muted,
    };
    container(
        row! {
            icon(match collapsed {
                true => MaterialSymbol::CHEVRON_RIGHT,
                false => MaterialSymbol::EXPAND_MORE,
            })
            .extent(Length::px(14.0))
            .color(ink);
            text(name.to_owned())
                .caption()
                .semibold()
                .no_wrap()
                .color(ink)
                .grow(1.0)
                .min_width(0.0);
        }
        .gap(Space::HAIR)
        .align_items(Align::End)
        .fill_width(),
    )
    .padding(Inset::edges(
        Space::ROOMY,
        Space::SNUG,
        Space::TIGHT,
        Space::HAIR,
    ))
    .on_press(move |chat: &mut Chat| chat.toggle_category(&toggled))
    .focusless()
    .height(CATEGORY)
    .fill_width()
    .any()
}

fn channel_row(channel: &ChannelLine) -> Element {
    let look = channel.look;
    let ink = look.ink();
    let voice = channel.kind.is_voice() && !channel.thread;
    let channel_id = channel.id;
    let symbol = match (channel.thread, channel.locked) {
        (true, _) => MaterialSymbol::SUBDIRECTORY_ARROW_RIGHT,
        (false, true) => MaterialSymbol::LOCK,
        (false, false) => kind_symbol(channel.kind),
    };
    let mut name = text(channel.name.clone()).label().no_wrap().color(ink);
    if look.bold() {
        name = name.semibold();
    } else {
        name = name.medium();
    }
    row_of([
        unread_dot(look),
        container(
            row! {
                icon(symbol).extent(Length::px(18.0)).color(ink.with_alpha(0.8));
                name.grow(1.0).min_width(0.0);
                if channel.nsfw {
                    icon(MaterialSymbol::EXPLICIT)
                        .extent(Length::px(14.0))
                        .color(palette().text_muted);
                }
                if channel.locked && channel.thread {
                    icon(MaterialSymbol::LOCK)
                        .extent(Length::px(14.0))
                        .color(palette().text_muted);
                }
                if let Some((occupied, limit)) = channel.seats {
                    text(format!("{occupied:02}/{limit:02}"))
                        .caption()
                        .no_wrap()
                        .color(palette().text_muted);
                }
                trailing_badge(look);
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .fill_width(),
        )
        .padding(Inset::edges(
            Space::NONE,
            Space::SNUG,
            Space::NONE,
            match channel.thread {
                true => Space::LOOSE,
                false => Space::SNUG,
            },
        ))
        .background(look.fill())
        .radius(Corner::Item)
        .on_press(move |chat: &mut Chat| match voice {
            true => chat.join_voice(&channel_id),
            false => chat.open_channel(&channel_id),
        })
        .focusless()
        .height(ROW)
        .grow(1.0)
        .any(),
    ])
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn voicer_row(voicer: &VoicerLine) -> Element {
    let muted = palette().text_muted;
    let mark = mark_of(("discord-voicer", &voicer.user));
    let (member, within) = (voicer.user, voicer.space);
    let account = voicer.account.clone();
    let stream = voicer.stream;
    let body = container(
        row! {
            component_with(
                |chat: &Chat| chat.calls(),
                (voicer.user, voicer.joined, voicer.picture.clone(), voicer.name.clone()),
                speaking_face,
            );
            text(voicer.name.clone())
                .caption()
                .no_wrap()
                .color(muted)
                .grow(1.0)
                .min_width(0.0);
            if let Some(key) = stream {
                live_badge()
                    .focusless()
                    .on_press(move |chat: &mut Chat| chat.watch_stream(&account, key));
            }
            if voicer.silenced {
                icon(MaterialSymbol::MIC_OFF).extent(Length::px(14.0)).color(muted);
            }
            if voicer.unhearing {
                icon(MaterialSymbol::HEADSET_OFF).extent(Length::px(14.0)).color(muted);
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::edges(
        Space::NONE,
        Space::SNUG,
        Space::NONE,
        Space::WIDE,
    ))
    .height(VOICER)
    .fill_width();
    context_area(
        mark,
        body,
        Handler::new(move |chat: &mut Chat, anchor: Anchor| {
            chat.open_member_menu(&member, Some(&within), anchor);
        }),
    )
}

fn speaking_face(calls: &Calls, props: &(UserId, bool, Option<Picture>, String)) -> Element {
    let (user, joined, picture, name) = props;
    let edge = match *joined && calls.is_speaking(user) {
        true => palette().success,
        false => Color::TRANSPARENT,
    };
    container(avatar(picture.as_ref(), name, VOICER_AVATAR))
        .border(2.0, edge)
        .radius(VOICER_AVATAR / 2.0 + 2.0)
        .padding(Inset::all(Space::HAIR))
        .any()
}
