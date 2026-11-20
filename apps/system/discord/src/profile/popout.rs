use discord_core::{Bond, UserId};
use rikka_app::prelude::*;
use rikka_ui::Handler;
use rikka_widgets::column;

use super::about::{basics, mutuals, roles};
use super::header::{Frame, action, header};
use super::identity::identity;
use super::look::{Look, Surface};
use super::{ProfileTarget, Profiles, Tab};
use crate::chat::Chat;
use crate::widgets::{mark_of, with_tooltip};

const NAME: f32 = 20.0;
const PANEL_NAME: f32 = 18.0;
const DETAILS_MOST: f32 = 420.0;
const ROLES_MOST: usize = 12;

pub(crate) fn popout(chat: &Chat) -> Option<Element> {
    let (target, anchor) = chat.profiles().popout()?.clone();
    let look = Look::resolve(chat, &target, Surface::Popout)?;
    let frame = Frame::POPOUT;
    let open_full = opener(&target);
    let mut details = vec![identity(&look, NAME, Some(open_full.clone()))];
    details.extend(basics(&look));
    details.extend(roles(&look, Some((ROLES_MOST, open_full.clone()))));
    details.extend(mutuals(&look));
    let details = column_of(details)
        .gap(Space::NORMAL)
        .padding(Inset::edges(
            Space::SNUG,
            Space::ROOMY,
            Space::NORMAL,
            Space::ROOMY,
        ))
        .fill_width();
    let body = column! {
        header(&look, frame, actions(&look), Some(open_full));
        scrollable(details)
            .shows(("discord-profile-popout", &target))
            .max_height(DETAILS_MOST)
            .fill_width();
        if !look.me {
            container(message_box(chat, &look))
                .padding(Inset::edges(Space::NONE, Space::ROOMY, Space::ROOMY, Space::ROOMY))
                .fill_width();
        }
    }
    .width(frame.width);
    Some(
        popover(
            container(body)
                .background(look.backdrop())
                .border(1.0, palette().outline)
                .radius(Corner::Panel),
        )
        .flush()
        .on_dismiss(|profiles: &mut Profiles| profiles.close_popout())
        .build()
        .anchored(anchor),
    )
}

pub(crate) fn panel(chat: &Chat, key: &str, user: &UserId, width: f32) -> Option<Element> {
    let target = ProfileTarget {
        key: key.to_owned(),
        user: *user,
        space: None,
    };
    let look = Look::resolve(chat, &target, Surface::Panel)?;
    let open_full = opener(&target);
    let mut details = vec![identity(&look, PANEL_NAME, Some(open_full.clone()))];
    details.extend(basics(&look));
    details.extend(mutuals(&look));
    details.push(
        button(text(tr!("discord.profile.view-full")))
            .secondary()
            .on_press(open_full.clone())
            .fill_width()
            .any(),
    );
    let body = column! {
        header(&look, Frame::panel(width), Vec::new(), Some(open_full));
        column_of(details)
            .gap(Space::NORMAL)
            .padding(Inset::edges(Space::SNUG, Space::ROOMY, Space::ROOMY, Space::ROOMY))
            .fill_width();
    }
    .width(width);
    Some(
        container(scrollable(body).fill())
            .background(look.backdrop())
            .fill()
            .any(),
    )
}

fn opener(target: &ProfileTarget) -> Handler<()> {
    let target = target.clone();
    Handler::new(move |chat: &mut Chat| chat.open_full_profile(&target, Tab::About))
}

pub(super) fn actions(look: &Look<'_>) -> Vec<Element> {
    let mut shown = Vec::new();
    if look.me {
        return shown;
    }
    let befriending = match look.bond {
        _ if look.user.bot => None,
        None => Some((
            MaterialSymbol::PERSON_ADD,
            tr!("discord.profile.add-friend"),
            true,
        )),
        Some(Bond::Incoming) => Some((
            MaterialSymbol::HOW_TO_REG,
            tr!("discord.profile.accept-friend"),
            true,
        )),
        Some(Bond::Outgoing) => Some((
            MaterialSymbol::HOURGLASS_TOP,
            tr!("discord.profile.request-sent"),
            false,
        )),
        Some(Bond::Friend | Bond::Blocked) => None,
    };
    if let Some((symbol, tip, pressable)) = befriending {
        let target = look.target.clone();
        let button = match pressable {
            true => action(symbol).on_press(move |chat: &mut Chat| chat.add_friend(&target)),
            false => action(symbol),
        };
        shown.push(with_tooltip(
            ("discord-profile-friend", look.mark()),
            button,
            tip,
        ));
    }
    let more = ("discord-profile-more", look.mark());
    let anchor = Anchor::new(mark_of(more), AnchorEdge::BelowEnd).offset(0.0, Space::TIGHT.px());
    let member = look.target.clone();
    shown.push(with_tooltip(
        more,
        action(MaterialSymbol::MORE_HORIZ)
            .on_press(move |chat: &mut Chat| chat.open_profile_menu(&member, anchor)),
        tr!("discord.profile.more"),
    ));
    shown
}

fn message_box(chat: &Chat, look: &Look<'_>) -> Element {
    let limits = look.linked.client().limits();
    text_field(chat.profiles().message())
        .placeholder(tr!(
            "discord.profile.message-to",
            name = look.user.handle.as_str()
        ))
        .max_len(limits.message_chars)
        .on_submit(|chat: &mut Chat| chat.send_from_profile())
        .fill_width()
        .any()
}
