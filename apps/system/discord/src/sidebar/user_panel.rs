use discord_core::Status;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::{Sidebar, account_menu};
use crate::chat::Chat;
use crate::settings::Section;
use crate::widgets::{avatar_with_status, status_label};

pub(crate) const MARK: &str = "discord-user-panel";
const HEIGHT: f32 = 56.0;
const FACE: f32 = 32.0;

pub(crate) fn view(chat: &Chat) -> Element {
    let Some(linked) = chat.current() else {
        return spacer().height(HEIGHT).any();
    };
    let ground = palette().background.mix(palette().surface, 0.5);
    let me = linked.me();
    let name = me.map_or_else(|| linked.account().name.clone(), |me| me.name().to_owned());
    let picture = me.and_then(|me| me.avatar.clone());
    let presence = linked.own_presence();
    let status = match presence.status {
        Status::Offline => Status::Online,
        status => status,
    };
    let below: Label = match &presence.mood {
        Some(mood) if !mood.is_empty() => mood.clone().into(),
        _ => status_label(status),
    };
    let muted = chat.muted();
    let deafened = chat.deafened();
    let mut layers = vec![
        container(
            row! {
                container(
                    row! {
                        avatar_with_status(picture.as_ref(), &name, FACE, status, ground);
                        column! {
                            text(name.clone()).label().semibold().no_wrap();
                            text(below).caption().muted().no_wrap();
                        }
                        .grow(1.0)
                        .min_width(0.0);
                    }
                    .gap(Space::SNUG)
                    .align_items(Align::Center)
                    .fill_width(),
                )
                .padding(Inset::xy(Space::TIGHT, Space::TIGHT))
                .radius(Corner::Item)
                .on_press(|sidebar: &mut Sidebar| sidebar.toggle_menu())
                .focusless()
                .grow(1.0)
                .min_width(0.0)
                .build()
                .keyed(MARK);
                toggle(
                    match muted {
                        true => MaterialSymbol::MIC_OFF,
                        false => MaterialSymbol::MIC,
                    },
                    muted,
                )
                .on_press(|chat: &mut Chat| chat.toggle_mute());
                toggle(
                    match deafened {
                        true => MaterialSymbol::HEADSET_OFF,
                        false => MaterialSymbol::HEADPHONES,
                    },
                    deafened,
                )
                .on_press(|chat: &mut Chat| chat.toggle_deafen());
                toggle(MaterialSymbol::SETTINGS, false)
                    .on_press(|chat: &mut Chat| chat.open_settings(Section::default()));
            }
            .gap(Space::HAIR)
            .align_items(Align::Center)
            .fill_width(),
        )
        .padding(Inset::xy(Space::SNUG, Space::NONE))
        .background(ground)
        .height(HEIGHT)
        .fill_width()
        .any(),
    ];
    if chat.sidebar().menu_open() {
        layers.push(account_menu::view(chat).anchored(Anchor::above(MARK).offset(0.0, -8.0)));
    }
    column_of(layers).fill_width().any()
}

fn toggle(symbol: MaterialSymbol, on: bool) -> Button {
    let tint = match on {
        true => palette().danger,
        false => palette().text_muted,
    };
    button(icon(symbol).extent(Length::px(20.0)).color(tint))
        .ghost()
        .icon_only()
        .small()
        .focusless()
}
