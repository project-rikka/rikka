use discord_core::{SavedAccount, Status};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::Sidebar;
use crate::brand;
use crate::chat::Chat;
use crate::widgets::{CHOSEN, status_color, status_label};

const WIDTH: f32 = 260.0;
const ENTRY: f32 = 36.0;

pub(crate) fn view(chat: &Chat) -> Element {
    let shown = chat
        .current()
        .map(|linked| linked.own_presence().status)
        .unwrap_or_default();
    let current = chat.current_key().unwrap_or_default().to_owned();
    let accounts = column_of(
        chat.accounts()
            .all()
            .iter()
            .map(|saved| account(chat, saved, saved.key == current)),
    )
    .gap(Space::HAIR)
    .fill_width();
    let signed_out = current.clone();
    popover(
        column! {
            column_of(CHOSEN.map(|status| status_entry(status, status == shown)))
                .gap(Space::HAIR)
                .fill_width();
            rule();
            heading(tr!("discord.accounts.title"));
            accounts;
            entry(MaterialSymbol::PERSON_ADD, tr!("discord.accounts.add"), palette().text)
                .on_press(|chat: &mut Chat| chat.add_account());
            rule();
            entry(MaterialSymbol::LOGOUT, tr!("discord.accounts.sign-out"), palette().danger)
                .on_press(move |chat: &mut Chat| chat.sign_out(&signed_out));
        }
        .gap(Space::TIGHT)
        .width(WIDTH),
    )
    .on_dismiss(|sidebar: &mut Sidebar| sidebar.close_menu())
    .build()
}

fn status_entry(status: Status, chosen: bool) -> Element {
    container(
        row! {
            container(spacer())
                .background(status_color(status))
                .radius(5.0)
                .size(Length::px(10.0), Length::px(10.0));
            text(status_label(status)).label().no_wrap().grow(1.0);
            if chosen {
                icon(MaterialSymbol::CHECK).extent(Length::px(16.0)).color(palette().accent);
            }
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .radius(Corner::Item)
    .on_press(move |chat: &mut Chat| chat.set_status(status))
    .focusless()
    .height(ENTRY)
    .fill_width()
    .any()
}

fn account(chat: &Chat, saved: &SavedAccount, current: bool) -> Element {
    let connection_error = chat.connection_error(&saved.key).is_some();
    let chosen = saved.key.clone();
    container(
        row! {
            icon(brand::mark()).extent(Length::px(18.0)).color(palette().text_muted);
            column! {
                text(saved.account.name.clone()).label().medium().no_wrap();
                if connection_error {
                    text(tr!("discord.accounts.connection-failed")).caption().muted().no_wrap();
                }
            }
            .grow(1.0)
            .min_width(0.0);
            if current {
                icon(MaterialSymbol::CHECK).extent(Length::px(16.0)).color(palette().accent);
            }
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::TIGHT))
    .radius(Corner::Item)
    .background(match current {
        true => palette().accent.with_alpha(0.12),
        false => Color::TRANSPARENT,
    })
    .on_press(move |chat: &mut Chat| chat.switch(&chosen))
    .focusless()
    .fill_width()
    .any()
}

fn entry(symbol: MaterialSymbol, label: Label, tint: Color) -> Container {
    container(
        row! {
            icon(symbol).extent(Length::px(18.0)).color(tint);
            text(label).label().no_wrap().color(tint).grow(1.0);
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

fn heading(title: Label) -> impl View {
    container(text(title).caption().semibold().muted().no_wrap())
        .padding(Inset::xy(Space::SNUG, Space::TIGHT))
        .fill_width()
}
