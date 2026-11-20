use rikka_mcp::{Server, ServerConfigError, ServerStatus, Transport};
use rikka_widgets::{column, row};

use crate::prelude::*;
use crate::servers::Titled;
use crate::servers::offered::Offered;
use crate::servers::offers::offers;
use crate::servers::state::Servers;

const GLYPH: f32 = 18.0;
const CHEVRON: f32 = 16.0;
const DOT: f32 = 8.0;
const GAP: f32 = 4.0;

const MORE: &str = "chat.servers.more";

#[derive(Clone, Copy)]
enum Choice {
    Move(usize),
    Copy(usize),
    SignIn,
    SignOut,
    Reconnect,
    Apart,
    Remove,
}

enum CardState<'a> {
    Off,
    Wrong(Label),
    Overridden(Label),
    Untried,
    Signing,
    Locked,
    Failed(&'a str),
    Reached(&'a ServerStatus),
}

pub(crate) fn card(server: &Server, servers: &Servers, titled: Titled) -> impl View {
    let name = server.name();
    let status = card_state(server, servers, titled);

    column! {
        heading(server, colour(&status), servers);
        text(server.transport().to_string()).caption().muted().no_wrap();
        match &status {
            CardState::Reached(standing) => connected_row(name, standing, servers).any(),
            CardState::Locked => signing(name, false).any(),
            CardState::Signing => signing(name, true).any(),
            other => text(status_label(other)).caption().color(colour(other)).any(),
        }
        if servers.shows_menu(name) {
            menu_panel(server, servers, titled);
        }
    }
    .gap(Space::TIGHT)
    .fill_width()
}

fn connected_row(name: &str, standing: &ServerStatus, servers: &Servers) -> Flex {
    let offered: Vec<(Offered, usize, bool)> = Offered::ALL
        .into_iter()
        .map(|kind| (kind, kind.count(standing), servers.is_unfolded(name, kind)))
        .filter(|(_, count, _)| *count > 0)
        .collect();
    column! {
        if let Some(title) = standing.title() {
            text(title.to_owned()).caption().muted();
        }
        if offered.is_empty() {
            text(tr!("chat.servers.toolless")).caption().color(palette().success);
        } else {
            row_of(offered.iter().map(|(kind, count, unfolded)| {
                unfolding(name, *kind, *count, *unfolded).any()
            }))
            .gap(Space::SNUG)
            .wrap_items();
            for (kind, _, unfolded) in &offered {
                if *unfolded {
                    offers(&kind.listed(standing));
                }
            }
        }
    }
    .gap(Space::TIGHT)
    .fill_width()
}

fn card_state<'a>(server: &Server, servers: &'a Servers, titled: Titled) -> CardState<'a> {
    if server.is_disabled() {
        return CardState::Off;
    }
    if let Some(nonsense) = server.config_error() {
        return CardState::Wrong(wrong(nonsense));
    }
    let Some(standing) = servers.standing(server.name()) else {
        return CardState::Untried;
    };
    if let Some(place) = standing.place()
        && place != servers.place().name()
    {
        return CardState::Overridden(tr!(
            "chat.servers.overridden",
            scope = &titled(place).to_string()
        ));
    }
    if servers.is_signing_in(server.name()) {
        return CardState::Signing;
    }
    if standing.needs_sign_in() {
        return CardState::Locked;
    }
    match standing.error() {
        Some(trouble) => CardState::Failed(trouble),
        None if standing.is_started() => CardState::Reached(standing),
        None => CardState::Off,
    }
}

fn colour(status: &CardState<'_>) -> Color {
    let palette = palette();
    match status {
        CardState::Off | CardState::Untried | CardState::Overridden(_) => palette.text_muted,
        CardState::Wrong(_) | CardState::Failed(_) => palette.danger,
        CardState::Locked | CardState::Signing => palette.warning,
        CardState::Reached(_) => palette.success,
    }
}

fn status_label(status: &CardState<'_>) -> Label {
    match status {
        CardState::Off => tr!("chat.servers.off"),
        CardState::Untried => tr!("chat.servers.untried"),
        CardState::Wrong(message) | CardState::Overridden(message) => message.clone(),
        CardState::Failed(trouble) => Label::from(*trouble),
        CardState::Locked => tr!("chat.servers.locked"),
        CardState::Signing => tr!("chat.servers.signing"),
        CardState::Reached(standing) => Offered::Tools.counted(standing.tool_count()),
    }
}

fn heading(server: &Server, colour: Color, servers: &Servers) -> impl View {
    let switching = server.name().to_owned();
    let editing = server.name().to_owned();
    let opening = server.name().to_owned();

    row! {
        container(spacer())
            .background(colour)
            .radius(DOT / 2.0)
            .width(DOT)
            .height(DOT);
        text(server.name()).label().medium();
        text(server.transport().kind()).caption().muted().grow(1.0);
        switch(!server.is_disabled())
            .control_size(ControlSize::Small)
            .on_toggle(move |servers: &mut Servers, on: bool| servers.enable(&switching, on));
        glyph(MaterialSymbol::EDIT)
            .enabled(!servers.is_drafting())
            .on_press(move |servers: &mut Servers| servers.edit(&editing))
            .tooltip(tr!("chat.servers.edit"));
        glyph(MaterialSymbol::MORE_VERT)
            .on_press(move |servers: &mut Servers| servers.open_menu(&opening))
            .build()
            .keyed((MORE, server.name()));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn glyph(symbol: MaterialSymbol) -> Button {
    button(
        icon(symbol)
            .extent(Length::px(GLYPH))
            .color(palette().text_muted),
    )
    .icon_only()
    .ghost()
    .small()
}

fn unfolding(name: &str, offered: Offered, count: usize, unfolded: bool) -> impl View {
    let folding = name.to_owned();
    let colour = palette().success;
    button(
        row! {
            text(offered.counted(count))
                .caption()
                .color(colour);
            icon(match unfolded {
                true => MaterialSymbol::EXPAND_LESS,
                false => MaterialSymbol::EXPAND_MORE,
            })
            .extent(Length::px(CHEVRON))
            .color(colour);
        }
        .gap(Space::HAIR)
        .align_items(Align::Center),
    )
    .ghost()
    .small()
    .on_press(move |servers: &mut Servers| servers.unfold(&folding, offered))
}

fn signing(name: &str, waiting: bool) -> impl View {
    let signing = name.to_owned();
    row! {
        text(match waiting {
            true => tr!("chat.servers.signing"),
            false => tr!("chat.servers.locked"),
        })
        .caption()
        .color(palette().warning)
        .grow(1.0);
        button(text(tr!("chat.servers.sign-in")).caption())
            .secondary()
            .small()
            .loading(waiting)
            .enabled(!waiting)
            .on_press(move |servers: &mut Servers| servers.sign_in(&signing));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn menu_panel(server: &Server, servers: &Servers, titled: Titled) -> Element {
    let name = server.name();
    let carriable = servers.carriable(name);
    let mut choices = Vec::new();
    for (at, place) in servers.places().iter().enumerate() {
        if at != servers.scope() {
            choices.push((Choice::Move(at), place.name()));
            choices.push((Choice::Copy(at), place.name()));
        }
    }
    let remote = matches!(server.transport(), Transport::Http(_));
    if remote {
        match servers.is_signed_in(name) {
            true => choices.push((Choice::SignOut, "")),
            false => choices.push((Choice::SignIn, "")),
        }
    }
    if !server.is_disabled() {
        choices.push((Choice::Reconnect, ""));
    }
    choices.push((Choice::Apart, ""));
    choices.push((Choice::Remove, ""));

    let items: Vec<MenuItem> = choices
        .iter()
        .map(|(choice, place)| match choice {
            Choice::Move(at) => {
                menu_item(tr!("chat.servers.move", scope = &titled(place).to_string()))
                    .icon(MaterialSymbol::DRIVE_FILE_MOVE)
                    .enabled(carriable.contains(at))
            }
            Choice::Copy(at) => {
                menu_item(tr!("chat.servers.copy", scope = &titled(place).to_string()))
                    .icon(MaterialSymbol::CONTENT_COPY)
                    .enabled(carriable.contains(at))
            }
            Choice::SignIn => menu_item(tr!("chat.servers.sign-in")).icon(MaterialSymbol::LOGIN),
            Choice::SignOut => menu_item(tr!("chat.servers.sign-out")).icon(MaterialSymbol::LOGOUT),
            Choice::Reconnect => {
                menu_item(tr!("chat.servers.reconnect")).icon(MaterialSymbol::REFRESH)
            }
            Choice::Apart => menu_separator(),
            Choice::Remove => menu_item(tr!("chat.servers.remove")).icon(MaterialSymbol::DELETE),
        })
        .collect();

    let selected_name = name.to_owned();
    let choices_list: Vec<Choice> = choices.iter().map(|(choice, _)| *choice).collect();
    menu()
        .items(items)
        .control_size(ControlSize::Small)
        .on_activate(move |servers: &mut Servers, row: usize| {
            let Some(choice) = choices_list.get(row).copied() else {
                return;
            };
            match choice {
                Choice::Move(to) => servers.carry(&selected_name, to, true),
                Choice::Copy(to) => servers.carry(&selected_name, to, false),
                Choice::SignIn => servers.sign_in(&selected_name),
                Choice::SignOut => servers.sign_out(&selected_name),
                Choice::Reconnect => servers.reconnect(&selected_name),
                Choice::Apart => servers.close_menu(),
                Choice::Remove => servers.remove(&selected_name),
            }
        })
        .on_dismiss(Servers::close_menu)
        .build()
        .anchored(Anchor::new((MORE, name), AnchorEdge::BelowEnd).offset(0.0, GAP))
}

fn wrong(nonsense: ServerConfigError) -> Label {
    match nonsense {
        ServerConfigError::MissingName => tr!("chat.servers.nameless"),
        ServerConfigError::MissingCommand => tr!("chat.servers.commandless"),
        ServerConfigError::MissingUrl => tr!("chat.servers.urlless"),
        _ => tr!("chat.servers.unwebbed"),
    }
}
