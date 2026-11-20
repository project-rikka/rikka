use discord_core::{Bond, Friend, Picture, Status, UserId};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::{Friends, Tab};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::widgets::{avatar_with_status, status_label};

const HEADER: f32 = 48.0;
const ROW: f32 = 62.0;
const FACE: f32 = 32.0;

#[derive(Clone, Debug)]
struct Line {
    key: String,
    user: UserId,
    name: String,
    handle: String,
    picture: Option<Picture>,
    status: Status,
    bond: Bond,
}

pub(crate) fn view(chat: &Chat) -> Element {
    let Some(linked) = chat.current() else {
        return spacer().fill().any();
    };
    let friends = chat.friends();
    let tab = friends.tab();
    let body = match tab {
        Tab::Add => adding(friends),
        listed => listing(linked, listed),
    };
    column! {
        header(tab);
        if let Some(outcome) = friends.outcome() {
            answer(outcome, tab);
        }
        body;
    }
    .fill()
    .any()
}

fn header(tab: Tab) -> impl View {
    let chosen = Tab::LISTED.iter().position(|listed| *listed == tab);
    container(
        row! {
            icon(MaterialSymbol::GROUP).extent(Length::px(22.0)).color(palette().text_muted);
            text(tr!("discord.sidebar.friends")).label().semibold().no_wrap();
            container(
                segmented(chosen.unwrap_or(usize::MAX))
                    .segments(Tab::LISTED.map(Tab::label))
                    .control_size(ControlSize::Small)
                    .on_select(|friends: &mut Friends, at: usize| {
                        friends.show(Tab::LISTED.get(at).copied().unwrap_or_default());
                    }),
            )
            .width(380.0);
            spacer().grow(1.0);
            button(text(Tab::Add.label()))
                .small()
                .primary()
                .on_press(|friends: &mut Friends| friends.show(Tab::Add));
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::ROOMY, Space::NONE))
    .align_y(Align::Center)
    .height(HEADER)
    .fill_width()
}

fn answer(outcome: &Result<(), String>, tab: Tab) -> Element {
    let (message, tint): (Label, Color) = match outcome {
        Ok(()) if tab == Tab::Add => (tr!("discord.friends.sent"), palette().success),
        Ok(()) => return spacer().height(0.0).any(),
        Err(trouble) => (trouble.clone().into(), palette().danger),
    };
    container(text(message).caption().color(tint).fill_width())
        .padding(Inset::xy(Space::ROOMY, Space::TIGHT))
        .fill_width()
        .any()
}

fn listing(linked: &LinkedAccount, tab: Tab) -> Element {
    let lines: Vec<Line> = linked
        .friends()
        .into_iter()
        .filter_map(|friend| line(linked, friend))
        .filter(|line| shows(tab, line))
        .collect();
    if lines.is_empty() {
        return status_page(MaterialSymbol::GROUP, tr!("discord.friends.none"))
            .fill()
            .any();
    }
    let count = lines.len() as i64;
    let heading = match tab {
        Tab::Online => tr!("discord.friends.online-count", count = count),
        Tab::Pending => tr!("discord.friends.pending-count", count = count),
        Tab::Blocked => tr!("discord.friends.blocked-count", count = count),
        Tab::All | Tab::Add => tr!("discord.friends.all-count", count = count),
    };
    column! {
        container(text(heading).caption().semibold().muted().no_wrap())
            .padding(Inset::edges(Space::ROOMY, Space::ROOMY, Space::SNUG, Space::ROOMY))
            .fill_width();
        lazy_column(lines.len(), move |slot| row_of_line(&lines[slot.index]))
            .padding(Inset::xy(Space::SNUG, Space::NONE))
            .shows(tab)
            .grow(1.0)
            .fill_width();
    }
    .grow(1.0)
    .fill_width()
    .any()
}

fn line(linked: &LinkedAccount, friend: &Friend) -> Option<Line> {
    let user = linked.user(&friend.user)?;
    Some(Line {
        key: linked.key().to_owned(),
        user: friend.user,
        name: friend
            .nickname
            .clone()
            .filter(|nickname| !nickname.is_empty())
            .unwrap_or_else(|| user.name().to_owned()),
        handle: user.handle.clone(),
        picture: user.avatar.clone(),
        status: linked
            .presence(&friend.user)
            .map_or(Status::Offline, |presence| presence.status),
        bond: friend.bond,
    })
}

fn shows(tab: Tab, line: &Line) -> bool {
    match tab {
        Tab::Online => line.bond == Bond::Friend && line.status.is_present(),
        Tab::All => line.bond == Bond::Friend,
        Tab::Pending => matches!(line.bond, Bond::Incoming | Bond::Outgoing),
        Tab::Blocked => line.bond == Bond::Blocked,
        Tab::Add => false,
    }
}

fn row_of_line(line: &Line) -> Element {
    let below: Label = match line.bond {
        Bond::Friend => status_label(line.status),
        Bond::Incoming => tr!("discord.friends.incoming"),
        Bond::Outgoing => tr!("discord.friends.outgoing"),
        Bond::Blocked => tr!("discord.friends.blocked-one"),
    };
    let actions: Vec<Element> = match line.bond {
        Bond::Friend => vec![
            action(
                MaterialSymbol::CHAT_BUBBLE,
                palette().text,
                line,
                Act::Message,
            ),
            action(
                MaterialSymbol::PERSON_REMOVE,
                palette().danger,
                line,
                Act::Remove,
            ),
        ],
        Bond::Incoming => vec![
            action(MaterialSymbol::CHECK, palette().success, line, Act::Accept),
            action(MaterialSymbol::CLOSE, palette().danger, line, Act::Remove),
        ],
        Bond::Outgoing | Bond::Blocked => {
            vec![action(
                MaterialSymbol::CLOSE,
                palette().danger,
                line,
                Act::Remove,
            )]
        }
    };
    container(
        row! {
            avatar_with_status(line.picture.as_ref(), &line.name, FACE, line.status, palette().surface);
            column! {
                row! {
                    text(line.name.clone()).label().semibold().no_wrap();
                    text(line.handle.clone()).caption().muted().no_wrap();
                }
                .gap(Space::SNUG)
                .align_items(Align::End);
                text(below).caption().muted().no_wrap();
            }
            .grow(1.0)
            .min_width(0.0);
            row_of(actions).gap(Space::SNUG);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .height(ROW)
    .fill_width()
    .any()
}

#[derive(Clone, Copy, Debug)]
enum Act {
    Message,
    Accept,
    Remove,
}

fn action(symbol: MaterialSymbol, tint: Color, line: &Line, act: Act) -> Element {
    let (key, user) = (line.key.clone(), line.user);
    button(icon(symbol).extent(Length::px(18.0)).color(tint))
        .secondary()
        .icon_only()
        .small()
        .focusless()
        .on_press(move |chat: &mut Chat| match act {
            Act::Message => chat.message_user(&key, &user),
            Act::Accept => chat.accept_friend(&user),
            Act::Remove => chat.unfriend(&user),
        })
        .any()
}

fn adding(friends: &Friends) -> Element {
    column! {
        text(tr!("discord.friends.add-title")).label().bold();
        text(tr!("discord.friends.add-hint")).caption().muted().fill_width();
        row! {
            text_field(friends.field())
                .placeholder(tr!("discord.friends.add-placeholder"))
                .autofocus(true)
                .on_submit(|chat: &mut Chat| chat.add_friend_by_name())
                .grow(1.0);
            button(text(tr!("discord.friends.send")))
                .primary()
                .on_press(|chat: &mut Chat| chat.add_friend_by_name());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::SNUG)
    .padding(Inset::all(Space::LOOSE))
    .fill_width()
    .any()
}
