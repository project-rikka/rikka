use discord_voice::MAX_VOLUME;
use rikka_app::prelude::*;
use rikka_widgets::{ImageFit, column, row};

use crate::chat::Chat;
use crate::screen::{Phase, Viewing};
use crate::widgets::{avatar, live_badge};

const FACE: f32 = 28.0;
const VOLUME: f32 = 120.0;

pub(crate) fn view(chat: &Chat) -> Element {
    let screens = chat.screens();
    let header = match screens.viewing() {
        Some(viewing) => watched_header(chat, viewing),
        None => own_header(chat),
    };
    let picture = match screens.viewing() {
        Some(viewing) => watched(chat, viewing),
        None => own(chat),
    };
    column! {
        header;
        container(picture)
            .background(Color::BLACK)
            .radius(Corner::Item)
            .grow(1.0)
            .fill_width();
    }
    .gap(Space::SNUG)
    .padding(Inset::all(Space::NORMAL))
    .fill()
    .any()
}

fn watched_header(chat: &Chat, viewing: &Viewing) -> Element {
    let linked = chat.linked(&viewing.account);
    let owner = viewing.key.owner;
    let name = linked
        .and_then(|linked| linked.name_of(viewing.key.space.as_ref(), &owner))
        .unwrap_or_else(|| owner.to_string());
    let picture = linked
        .and_then(|linked| linked.user(&owner))
        .and_then(|user| user.avatar.clone());
    let volume = viewing.volume;
    row! {
        avatar(picture.as_ref(), &name, FACE);
        text(name.clone()).label().semibold().no_wrap().min_width(0.0);
        live_badge();
        watchers(viewing.viewers.len());
        spacer().grow(1.0);
        icon(MaterialSymbol::VOLUME_UP).extent(Length::px(18.0)).color(palette().text_muted);
        slider(volume)
            .range(0.0..=MAX_VOLUME)
            .step(0.05)
            .on_change(|chat: &mut Chat, volume: f32| chat.set_stream_volume(volume))
            .width(VOLUME);
        button(icon(MaterialSymbol::CHAT).extent(Length::px(18.0)))
            .ghost()
            .icon_only()
            .small()
            .focusless()
            .on_press(|chat: &mut Chat| chat.show_stage(false));
        button(
            row! {
                icon(MaterialSymbol::CLOSE).extent(Length::px(16.0)).color(palette().danger);
                text(tr!("discord.screen.leave")).label().color(palette().danger);
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center),
        )
        .ghost()
        .small()
        .focusless()
        .on_press(|chat: &mut Chat| chat.stop_watching());
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn own_header(chat: &Chat) -> Element {
    let viewers = chat
        .screens()
        .sharing()
        .map_or(0, |sharing| sharing.viewers.len());
    row! {
        icon(MaterialSymbol::SCREEN_SHARE).extent(Length::px(20.0)).color(palette().accent);
        text(tr!("discord.screen.your-stream")).label().semibold().no_wrap();
        live_badge();
        watchers(viewers);
        spacer().grow(1.0);
        button(icon(MaterialSymbol::CHAT).extent(Length::px(18.0)))
            .ghost()
            .icon_only()
            .small()
            .focusless()
            .on_press(|chat: &mut Chat| chat.show_stage(false));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn watchers(count: usize) -> impl View {
    row! {
        icon(MaterialSymbol::VISIBILITY).extent(Length::px(16.0)).color(palette().text_muted);
        text(count.to_string()).caption().muted().no_wrap();
    }
    .gap(Space::HAIR)
    .align_items(Align::Center)
}

fn watched(chat: &Chat, viewing: &Viewing) -> Element {
    if viewing.phase != Phase::Live || viewing.size.is_none() {
        return waiting(tr!("discord.screen.connecting"));
    }
    let picture = image(chat.screens().picture())
        .fit(ImageFit::Contain)
        .fill()
        .any();
    if !viewing.paused {
        return picture;
    }
    column_of([picture, notice(tr!("discord.screen.paused"))])
        .fill()
        .any()
}

fn own(chat: &Chat) -> Element {
    let Some(sharing) = chat.screens().sharing() else {
        return spacer().fill().any();
    };
    if sharing.phase != Phase::Live || sharing.size.is_none() {
        return waiting(tr!("discord.screen.starting"));
    }
    let picture = image(chat.screens().thumbnail())
        .fit(ImageFit::Contain)
        .fill()
        .any();
    match (sharing.paused, sharing.watched) {
        (true, _) => column_of([picture, notice(tr!("discord.screen.paused"))])
            .fill()
            .any(),
        (false, false) => column_of([picture, notice(tr!("discord.screen.nobody"))])
            .fill()
            .any(),
        (false, true) => picture,
    }
}

fn waiting(caption: Label) -> Element {
    container(
        column! {
            spinner();
            text(caption).label().color(Color::WHITE.with_alpha(0.8));
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center),
    )
    .center()
    .fill()
    .any()
}

fn notice(caption: Label) -> Element {
    container(
        container(text(caption).label().semibold().color(Color::WHITE))
            .padding(Inset::xy(Space::NORMAL, Space::SNUG))
            .background(Color::BLACK.with_alpha(0.6))
            .radius(Corner::Control),
    )
    .center()
    .position(0.0, 0.0)
    .fill()
    .any()
}
