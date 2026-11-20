use discord_core::{Channel, ChannelId, ChannelKind, Picture, Status};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::rows::{Look, ROW, trailing_badge, unread_dot};
use crate::account::{LinkedAccount, Scope};
use crate::chat::Chat;
use crate::widgets::{avatar, avatar_with_status};

const DIRECT: f32 = 44.0;
const DIRECT_AVATAR: f32 = 32.0;
const HEADING: f32 = 36.0;

#[derive(Clone, Debug)]
enum Line {
    Friends { selected: bool },
    Heading,
    Direct(DirectLine),
}

#[derive(Clone, Debug)]
struct DirectLine {
    id: ChannelId,
    title: String,
    group: bool,
    members: usize,
    picture: Option<Picture>,
    status: Status,
    look: Look,
}

pub(crate) fn view(chat: &Chat) -> Element {
    let Some(linked) = chat.current() else {
        return spacer().any();
    };
    let lines = lines(linked, chat.sidebar().query());
    lazy_column(lines.len(), move |slot| line(&lines[slot.index]))
        .padding(Inset::xy(Space::SNUG, Space::SNUG))
        .shows("home")
        .fill_width()
        .grow(1.0)
        .any()
}

fn lines(linked: &LinkedAccount, query: &str) -> Vec<Line> {
    let location = linked.location();
    let selected = match location.scope {
        Scope::Home => location.channel,
        Scope::Space(_) => None,
    };
    let mut lines = vec![
        Line::Friends {
            selected: location.scope == Scope::Home && selected.is_none(),
        },
        Line::Heading,
    ];
    lines.extend(
        linked
            .directs()
            .into_iter()
            .map(|channel| direct_line(linked, channel, selected.as_ref()))
            .filter(|line| query.is_empty() || line.title.to_lowercase().contains(query))
            .map(Line::Direct),
    );
    lines
}

fn direct_line(
    linked: &LinkedAccount,
    channel: &Channel,
    selected: Option<&ChannelId>,
) -> DirectLine {
    let group = channel.kind == ChannelKind::Group;
    let peer = channel.recipients.first();
    let picture = match group {
        true => channel.icon.clone(),
        false => peer
            .and_then(|user| linked.user(user))
            .and_then(|user| user.avatar.clone()),
    };
    let status = match group {
        true => Status::Offline,
        false => peer
            .and_then(|user| linked.presence(user))
            .map_or(Status::Offline, |presence| presence.status),
    };
    DirectLine {
        id: channel.id,
        title: linked.channel_title(channel),
        group,
        members: channel.recipients.len() + 1,
        picture,
        status,
        look: Look {
            selected: selected == Some(&channel.id),
            unseen: linked.unseen(&channel.id),
            muted: channel.muted,
        },
    }
}

fn line(line: &Line) -> Element {
    match line {
        Line::Friends { selected } => friends(*selected),
        Line::Heading => container(
            text(tr!("discord.sidebar.direct-messages"))
                .caption()
                .semibold()
                .no_wrap()
                .muted(),
        )
        .padding(Inset::edges(
            Space::ROOMY,
            Space::SNUG,
            Space::TIGHT,
            Space::SNUG,
        ))
        .align_y(Align::End)
        .height(HEADING)
        .fill_width()
        .any(),
        Line::Direct(direct) => direct_row(direct),
    }
}

fn friends(selected: bool) -> Element {
    let look = Look {
        selected,
        ..Look::default()
    };
    container(
        row! {
            icon(MaterialSymbol::GROUP).extent(Length::px(22.0)).color(look.ink());
            text(tr!("discord.sidebar.friends")).label().medium().no_wrap().color(look.ink());
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::NORMAL, Space::NONE))
    .background(look.fill())
    .radius(Corner::Item)
    .on_press(|chat: &mut Chat| chat.open_friends())
    .focusless()
    .height(DIRECT)
    .fill_width()
    .any()
}

fn direct_row(direct: &DirectLine) -> Element {
    let direct = direct.clone();
    local_state(false, move |hovered: bool, set: Setter<bool>| {
        let look = direct.look;
        let ink = look.ink();
        let (to_open, to_close) = (direct.id, direct.id);
        let face = match direct.group {
            true => avatar(direct.picture.as_ref(), &direct.title, DIRECT_AVATAR),
            false => avatar_with_status(
                direct.picture.as_ref(),
                &direct.title,
                DIRECT_AVATAR,
                direct.status,
                palette().surface,
            ),
        };
        let mut title = text(direct.title.clone()).label().no_wrap().color(ink);
        title = match look.bold() {
            true => title.semibold(),
            false => title.medium(),
        };
        let fill = match (look.selected, hovered) {
            (false, true) => palette().text.with_alpha(0.05),
            _ => look.fill(),
        };
        row_of([
            unread_dot(look),
            container(
                row! {
                    face;
                    column! {
                        title;
                        if direct.group {
                            text(tr!("discord.sidebar.members", count = direct.members as i64))
                                .caption()
                                .no_wrap()
                                .muted();
                        }
                    }
                    .grow(1.0)
                    .min_width(0.0);
                    match hovered {
                        true => Some(
                            button(
                                icon(MaterialSymbol::CLOSE)
                                    .extent(Length::px(16.0))
                                    .color(palette().text_muted),
                            )
                            .ghost()
                            .icon_only()
                            .small()
                            .focusless()
                            .on_press(move |chat: &mut Chat| chat.close_direct(&to_close))
                            .any(),
                        ),
                        false => trailing_badge(look),
                    };
                }
                .gap(Space::NORMAL)
                .align_items(Align::Center)
                .fill_width(),
            )
            .padding(Inset::xy(Space::SNUG, Space::NONE))
            .background(fill)
            .radius(Corner::Item)
            .on_press(move |chat: &mut Chat| chat.open_channel(&to_open))
            .on_hover(move |inside: bool| {
                set(inside);
                skip_view();
            })
            .focusless()
            .height(DIRECT.max(ROW))
            .grow(1.0)
            .any(),
        ])
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
    })
}
