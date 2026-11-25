use discord_core::ChannelKind;
use discord_voice::InputMode;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::account::Scope;
use crate::chat::{Chat, Stage};
use crate::voice::talk_area;
use crate::{
    banner, channel_admin, conversation, friends, manage, member_menu, members, menus, profile,
    rail, screen, settings, sidebar, signing, space_admin, space_sheet, switcher, voice,
};

pub(crate) const BODY: &str = "discord-desk";

pub(crate) fn view(chat: &Chat) -> Element {
    match chat.stage() {
        Stage::SignIn => component(|chat: &Chat| chat, signing::view).fill().any(),
        Stage::Connecting => component(|chat: &Chat| chat, connecting).fill().any(),
        Stage::Desk if chat.channel_admin().is_open() => {
            component(|chat: &Chat| chat, channel_admin::view)
                .fill()
                .any()
        }
        Stage::Desk if chat.space_admin().is_open() => {
            component(|chat: &Chat| chat, admin_desk).fill().any()
        }
        Stage::Desk if chat.settings().is_open() => {
            component(|chat: &Chat| chat, settings::view).fill().any()
        }
        Stage::Desk => desk(chat),
    }
}

fn admin_desk(chat: &Chat) -> Element {
    column_of(std::iter::once(space_admin::view(chat)).chain(member_menu::view(chat)))
        .fill()
        .any()
}

fn desk(chat: &Chat) -> Element {
    let talking = matches!(chat.voice_settings().mode, InputMode::PushToTalk { .. })
        && chat.calls().seat().is_some();
    let body = row! {
        component(|chat: &Chat| chat, rail::view);
        component(|chat: &Chat| chat, sidebar::view);
        column! {
            component(|chat: &Chat| chat, banner::view);
            component(|chat: &Chat| chat, region).grow(1.0).fill_width();
        }
        .background(palette().surface.mix(palette().surface_variant, 0.35))
        .grow(1.0)
        .fill_height();
    }
    .fill();
    let mut layers = vec![body.build().keyed(BODY)];
    if chat.profiles().full().is_some() {
        layers.push(profile::scrim());
    }
    if chat.space_sheet().is_open() {
        layers.push(space_sheet::scrim());
    }
    layers.push(component(|chat: &Chat| chat, overlays).any());
    layers.push(
        component(|chat: &Chat| chat, switcher::view)
            .position(0.0, 0.0)
            .fill_width()
            .any(),
    );
    if chat.conversation().overlay().gallery().is_some() {
        layers.push(
            component(|chat: &Chat| chat, conversation::lightbox::view)
                .position(0.0, 0.0)
                .fill()
                .transition(fade())
                .anchored(Anchor::covering(BODY)),
        );
    }
    let layers = column_of(layers).fill();
    talk_area(talking.then(|| chat.talk_key()), layers)
}

fn region(chat: &Chat) -> Element {
    if chat.screens().stage_shown() {
        return component(|chat: &Chat| chat, screen::stage_view)
            .fill()
            .any();
    }
    let Some(linked) = chat.current() else {
        return spacer().fill().any();
    };
    let location = linked.location();
    let Some(channel) = location.channel else {
        return match location.scope {
            Scope::Home => component(|chat: &Chat| chat, friends::view).fill().any(),
            Scope::Space(_) => component(|chat: &Chat| chat, conversation::view)
                .fill()
                .any(),
        };
    };
    let listed = chat.conversation().members_shown()
        && linked.channel(&channel).is_some_and(|channel_info| {
            channel_info.kind.holds_messages() && channel_info.kind != ChannelKind::Voice
        });
    row! {
        component(|chat: &Chat| chat, conversation::view).grow(1.0).fill_height();
        if listed {
            component_with(|chat: &Chat| chat, channel, members::view);
        }
    }
    .fill()
    .any()
}

fn overlays(chat: &Chat) -> Element {
    let incoming = voice::incoming_view(chat)
        .map(|incoming| incoming.anchored(Anchor::new(BODY, AnchorEdge::Center)));
    let mut layers: Vec<Element> = [
        incoming,
        screen::golive_view(chat),
        Some(component(|chat: &Chat| chat, profile::full).any()),
        Some(component(|chat: &Chat| chat, space_sheet::view).any()),
        Some(component(|chat: &Chat| chat, profile::popout).any()),
        member_menu::view(chat),
        menus::view(chat),
    ]
    .into_iter()
    .flatten()
    .collect();
    layers.extend(manage::overlays(chat, BODY));
    column_of(layers).any()
}

fn connecting(chat: &Chat) -> Element {
    let name = chat
        .pending_account()
        .map(|saved| saved.account.name.clone())
        .unwrap_or_default();
    container(
        column! {
            match chat.error_message() {
                Some(_) => icon(MaterialSymbol::CLOUD_OFF)
                    .extent(Length::px(40.0))
                    .color(palette().danger)
                    .any(),
                None => spinner().any(),
            };
            text(tr!("discord.connecting.to", name = name.as_str())).heading().semibold();
            if let Some(error_message) = chat.error_message() {
                text(error_message.to_owned())
                    .caption()
                    .danger()
                    .align(TextAlign::Center)
                    .fill_width();
            }
            row! {
                if chat.error_message().is_some() {
                    button(text(tr!("discord.connecting.retry")))
                        .primary()
                        .on_press(|chat: &mut Chat| chat.retry());
                }
                button(text(tr!("discord.connecting.other")))
                    .ghost()
                    .on_press(|chat: &mut Chat| chat.add_account());
            }
            .gap(Space::SNUG);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .width(420.0),
    )
    .center()
    .fill()
    .any()
}

fn back_bar(title: Label, back: fn(&mut Chat)) -> CaptionBar {
    caption()
        .leading(
            button(
                row! {
                    icon(MaterialSymbol::ARROW_BACK).extent(Length::px(16.0));
                    text(tr!("discord.settings.back")).label();
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center),
            )
            .ghost()
            .small()
            .on_press(back),
        )
        .center(text(title).label().medium().no_wrap())
        .into()
}

pub(crate) fn caption_bar(chat: &Chat) -> Option<CaptionBar> {
    if chat.channel_admin().is_open() {
        return Some(back_bar(
            tr!("discord.channel-admin.title"),
            Chat::close_channel_admin,
        ));
    }
    if chat.space_admin().is_open() {
        return Some(back_bar(
            tr!("discord.admin.title"),
            Chat::close_space_admin,
        ));
    }
    if chat.settings().is_open() {
        return Some(back_bar(
            tr!("discord.settings.title"),
            Chat::close_settings,
        ));
    }
    let title = chat
        .current()
        .map(|linked| {
            let location = linked.location();
            let channel = location
                .channel
                .as_ref()
                .and_then(|channel| linked.channel(channel))
                .map(|channel_info| linked.channel_title(channel_info));
            let space = location
                .scope
                .space()
                .and_then(|space| linked.space(space))
                .map(|space_info| space_info.name.clone());
            match (space, channel) {
                (Some(space), Some(channel)) => format!("{channel} · {space}"),
                (Some(space), None) => space,
                (None, Some(channel)) => channel,
                (None, None) => tr!("discord.title").to_string(),
            }
        })
        .unwrap_or_else(|| tr!("discord.title").to_string());
    Some(
        caption()
            .leading(
                row! {
                    icon(MaterialSymbol::FORUM).extent(Length::px(16.0)).color(palette().accent);
                    text(title).label().medium().no_wrap();
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            )
            .into(),
    )
}
