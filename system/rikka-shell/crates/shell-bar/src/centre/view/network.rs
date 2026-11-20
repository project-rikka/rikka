use rikka_network::{AccessPoint, Security, State as NetworkState};
use rikka_ui::{
    Align, Element, Inset, Label, Length, MaterialSymbol, Space, Styled, TextStyled, View, palette,
    text, tr,
};
use rikka_widgets::{
    ButtonVariant, ControlSize, Switch, button, column_of, icon, row, scrollable, switch,
    text_field,
};
use shell_feeds::symbols;

use crate::centre::detail::Detail;
use crate::centre::state::Centre;
use crate::centre::view::quick::{Quick, quick};
use crate::centre::view::tile::{self, DETAIL, ICON, INSET};

const LIST: f32 = tile::ROW * 6.0;

const NETWORKS: &str = "centre.networks";

pub(crate) fn pressable(network: &NetworkState) -> bool {
    network.radio().is_some() && !network.is_killed()
}

pub(crate) fn tile(network: &NetworkState) -> Quick {
    let on = network.is_wireless_on();
    let joined = network.joined();

    quick(symbols::wireless(on, joined), Detail::Network.title())
        .taskbar_visibility(taskbar_visibility(on, joined))
        .on(on)
        .here(pressable(network))
        .toggle(move |centre: &mut Centre| centre.daemons().network().use_wireless(!on))
        .opens(Detail::Network)
}

pub(crate) fn radio(network: &NetworkState) -> Switch {
    switch(network.is_wireless_on())
        .control_size(ControlSize::Medium)
        .enabled(network.radio().is_some())
        .on_toggle(|centre: &mut Centre, on| centre.daemons().network().use_wireless(on))
}

pub(crate) fn page(centre: &Centre, network: &NetworkState) -> Vec<Element> {
    let mut rows = Vec::new();
    if network.is_wireless_on() {
        rows.push(
            scrollable(column_of(networks(centre, &network.networks())))
                .max_height(LIST)
                .build()
                .keyed(NETWORKS),
        );
    }
    if let Some(error) = &network.error {
        rows.push(tile::refusal(&error.reason).build());
    }
    rows
}

fn taskbar_visibility(on: bool, joined: Option<&AccessPoint>) -> Label {
    match (on, joined) {
        (false, _) => tr!("shell.centre.off"),
        (true, Some(point)) => Label::from(point.ssid.to_string()),
        (true, None) => tr!("shell.centre.not-joined"),
    }
}

fn networks(centre: &Centre, event: &[&AccessPoint]) -> Vec<Element> {
    let joining_ap = centre.joining();
    let mut rows = Vec::new();
    for point in event {
        rows.push(line(point));
        if joining_ap == Some(&point.id) {
            rows.push(passphrase(centre).build());
        }
    }
    if rows.is_empty() {
        rows.push(tile::caption(tr!("shell.centre.looking")).build());
    }
    rows
}

fn line(point: &AccessPoint) -> Element {
    let palette = palette();
    let id = point.id.clone();
    let mut line = tile::line(
        symbols::bars(point.bars()),
        Label::from(point.ssid.to_string()),
    )
    .on_press(move |centre: &mut Centre| centre.join(&id));

    if point.in_use {
        line = line
            .tint(palette.accent)
            .detail(Some(tr!("shell.centre.joined")));
    }
    if !point.security.is_open() {
        line = line.with(
            icon(lock(point.security))
                .extent(Length::px(ICON - 4.0))
                .color(palette.text_muted),
        );
    }
    if point.in_use {
        line = line.with(
            button(text(tr!("shell.centre.leave")).text_size(DETAIL))
                .variant(ButtonVariant::Ghost)
                .control_size(ControlSize::Small)
                .on_press(|centre: &mut Centre| centre.daemons().network().leave()),
        );
    }
    line.build()
}

fn lock(security: Security) -> MaterialSymbol {
    match security.needs_account() {
        true => MaterialSymbol::VPN_KEY,
        false => MaterialSymbol::LOCK,
    }
}

fn passphrase(centre: &Centre) -> impl View {
    row! {
        text_field(centre.passphrase())
            .placeholder(tr!("shell.centre.passphrase"))
            .control_size(ControlSize::Small)
            .secret(true)
            .autofocus(true)
            .on_change(|| {})
            .on_submit(Centre::confirm)
            .grow(1.0);
        button(text(tr!("shell.centre.join")).text_size(DETAIL))
            .control_size(ControlSize::Small)
            .on_press(Centre::confirm);
        button(icon(MaterialSymbol::CLOSE).extent(Length::px(ICON - 4.0)))
            .variant(ButtonVariant::Ghost)
            .icon_only()
            .control_size(ControlSize::Small)
            .on_press(Centre::cancel)
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .padding(Inset::xy(INSET, 4.0))
}
