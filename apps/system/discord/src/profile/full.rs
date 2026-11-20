use discord_core::User;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::about::{basics, connections, note, roles};
use super::header::{Frame, header};
use super::identity::identity;
use super::look::{Look, Surface};
use super::popout::actions;
use super::{ProfileTarget, Profiles, Tab};
use crate::chat::Chat;
use crate::desk::BODY;
use crate::widgets::{avatar, picture_tile};

const NAME: f32 = 24.0;
const ROOM: f32 = 280.0;
const ROW: f32 = 52.0;
const FACE: f32 = 40.0;

pub(crate) fn full(chat: &Chat) -> Option<Element> {
    let (target, tab) = chat.profiles().full()?.clone();
    let look = Look::resolve(chat, &target, Surface::Full)?;
    let frame = Frame::FULL;
    let tabs: Vec<Tab> = match look.me {
        true => vec![Tab::About],
        false => Tab::ALL.to_vec(),
    };
    let chosen = tabs.iter().position(|shown| *shown == tab).unwrap_or(0);
    let tab = tabs[chosen];
    let labels: Vec<Label> = tabs.iter().map(|shown| label(&look, *shown)).collect();
    let messaged = target.clone();
    let body = column! {
        header(&look, frame, actions(&look), None);
        column! {
            row! {
                identity(&look, NAME, None).grow(1.0).min_width(0.0);
                if !look.me {
                    button(
                        row! {
                            icon(MaterialSymbol::CHAT_BUBBLE).extent(Length::px(16.0));
                            text(tr!("discord.profile.send-message")).label();
                        }
                        .gap(Space::TIGHT)
                        .align_items(Align::Center),
                    )
                    .primary()
                    .on_press(move |chat: &mut Chat| chat.message_user(&messaged.key, &messaged.user));
                }
            }
            .gap(Space::NORMAL)
            .align_items(Align::Start)
            .fill_width();
            if tabs.len() > 1 {
                segmented(chosen)
                    .segments(labels)
                    .on_select(move |profiles: &mut Profiles, at: usize| {
                        profiles.show_tab(tabs.get(at).copied().unwrap_or_default());
                    })
                    .fill_width();
            }
            scrollable(shown_tab(chat, &look, tab))
                .shows(("discord-full-profile", &target, tab))
                .height(ROOM)
                .fill_width();
        }
        .gap(Space::NORMAL)
        .padding(Inset::edges(Space::SNUG, frame.inset, frame.inset, frame.inset))
        .fill_width();
    }
    .width(frame.width);
    let body = container(body)
        .background(look.backdrop())
        .radius(Corner::Panel);
    Some(
        sheet(body)
            .width(frame.width)
            .on_dismiss(|chat: &mut Chat| chat.close_full_profile())
            .keyed(("discord-full-profile", target))
            .centered_on(BODY)
            .build(),
    )
}

pub(crate) fn scrim() -> Element {
    rikka_widgets::scrim()
        .on_dismiss(|chat: &mut Chat| chat.close_full_profile())
        .transition(fade())
        .anchored(Anchor::covering(BODY))
}

fn label(look: &Look<'_>, tab: Tab) -> Label {
    match tab {
        Tab::About => tr!("discord.profile.about"),
        Tab::Servers => tr!(
            "discord.profile.mutual-servers",
            count = look.mutual_spaces().len() as i64
        ),
        Tab::Friends => tr!(
            "discord.profile.mutual-friends",
            count = look.mutual_friends().len() as i64
        ),
    }
}

fn shown_tab(chat: &Chat, look: &Look<'_>, tab: Tab) -> Element {
    match tab {
        Tab::About => {
            let mut parts = basics(look);
            parts.extend(roles(look, None));
            parts.extend(connections(look));
            if !look.me {
                parts.push(note(chat.profiles()));
            }
            column_of(parts)
                .gap(Space::ROOMY)
                .padding(Inset::xy(Space::NONE, Space::SNUG))
                .fill_width()
                .any()
        }
        _ if look.profile.is_none() => container(spinner())
            .center()
            .height(ROOM)
            .fill_width()
            .any(),
        Tab::Servers => listing(
            look.mutual_spaces().into_iter().map(space_row).collect(),
            tr!("discord.profile.no-mutual-servers"),
        ),
        Tab::Friends => listing(
            look.mutual_friends()
                .iter()
                .map(|friend| friend_row(&look.target, friend))
                .collect(),
            tr!("discord.profile.no-mutual-friends"),
        ),
    }
}

fn listing(rows: Vec<Element>, empty: Label) -> Element {
    if rows.is_empty() {
        return container(text(empty).label().muted())
            .center()
            .height(ROOM)
            .fill_width()
            .any();
    }
    column_of(rows).gap(Space::HAIR).fill_width().any()
}

fn space_row(space: &discord_core::Space) -> Element {
    let space_id = space.id;
    entry(
        picture_tile(
            space.icon.as_ref(),
            &space.name,
            FACE,
            Corner::Item.radius(),
        ),
        text(space.name.clone()).label().semibold().no_wrap().any(),
    )
    .on_press(move |chat: &mut Chat| chat.open_mutual_space(&space_id))
    .any()
}

fn friend_row(target: &ProfileTarget, friend: &User) -> Element {
    let shown = ProfileTarget {
        key: target.key.clone(),
        user: friend.id,
        space: None,
    };
    entry(
        avatar(friend.avatar.as_ref(), friend.name(), FACE),
        column! {
            text(friend.name().to_owned()).label().semibold().no_wrap();
            text(friend.handle.clone()).caption().muted().no_wrap();
        }
        .min_width(0.0)
        .any(),
    )
    .on_press(move |chat: &mut Chat| chat.open_full_profile(&shown, Tab::About))
    .any()
}

fn entry(face: Element, caption: Element) -> Container {
    container(
        row! {
            face;
            caption.grow(1.0).min_width(0.0);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .radius(Corner::Item)
    .focusless()
    .height(ROW)
    .fill_width()
}
