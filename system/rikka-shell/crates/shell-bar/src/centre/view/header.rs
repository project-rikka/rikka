use std::time::Duration;

use rikka_battery::{Charge, Reading};
use rikka_ui::{
    Align, Element, Inset, Label, Length, MaterialSymbol, Space, Styled, TextStyled, View, palette,
    text, tr,
};
use rikka_widgets::{column, icon, row, spacer};
use shell_feeds::symbols;

use crate::centre::state::Centre;
use crate::centre::view::{Readings, power, tile};

pub(crate) fn view(readings: &Readings) -> Element {
    let line = row! {
        for action in power::actions(readings.power.capabilities) {
            action;
        }
        spacer().grow(1.0);
        if let Some(battery) = &readings.battery {
            readout(battery);
        }
        tile::round(MaterialSymbol::SETTINGS, Centre::settings)
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .padding(Inset::xy(tile::PADDING, 0.0));
    match &readings.power.error {
        Some(refused) => column! {
            line;
            tile::refusal(&refused.reason)
        }
        .fill_width()
        .build(),
        None => line.build(),
    }
}

fn readout(battery: &Reading) -> impl View {
    let palette = palette();
    let charge = (battery.charge * 100.0).round() as u8;
    row! {
        icon(symbols::battery(battery))
            .extent(Length::px(tile::ICON))
            .color(match battery.state {
                Charge::Charging => palette.success,
                _ if battery.charge <= 0.1 => palette.danger,
                _ => palette.text,
            });
        text(format!("{charge}%")).label().medium();
        text(taskbar_visibility(battery))
            .text_size(tile::DETAIL)
            .muted()
            .no_wrap()
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn taskbar_visibility(battery: &Reading) -> Label {
    match (battery.state, battery.left) {
        (Charge::Full, _) => tr!("shell.centre.charged"),
        (Charge::Charging, None) => tr!("shell.centre.charging"),
        (Charge::Charging, Some(left)) => {
            tr!("shell.centre.until-full", time = clock(left).as_str())
        }
        (_, Some(left)) => tr!("shell.centre.left", time = clock(left).as_str()),
        (_, None) => Label::from(""),
    }
}

fn clock(left: Duration) -> String {
    let minutes = left.as_secs() / 60;
    format!("{}:{:02}", minutes / 60, minutes % 60)
}
