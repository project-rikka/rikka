use rikka_types::Color;
use rikka_ui::{
    Align, Inset, Length, MaterialSymbol, Space, TextStyled, View, palette, text, theme,
};
use rikka_widgets::{container, icon, row};
use shell_feeds::{Daemons, percent, symbols};

use crate::cluster::{DEPTH, INSET, PITCH, RADIUS, SYMBOL};
use crate::state::Bar;
use crate::status::bell::Bell;

pub(crate) fn status(
    daemons: &Daemons,
    bell: Option<Bell>,
    percentage: bool,
    hovered: bool,
) -> impl View {
    let graph = daemons.sound().state();
    let network = daemons.network().state();
    let palette = palette();
    container(
        row! {
            if let Some(battery) = daemons.battery().main() {
                row! {
                    symbol(symbols::battery(&battery));
                    if percentage {
                        text(percent(battery.charge))
                            .label()
                            .no_wrap()
                            .color(palette.text);
                    }
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center);
            }
            if let Some(source) = graph.source() {
                symbol(symbols::microphone(Some(&source.volume)));
            }
            if let Some(sink) = graph.sink() {
                symbol(symbols::sound(Some(&sink.volume)));
            }
            if network.radio().is_some() {
                symbol(symbols::wireless(network.is_wireless_on(), network.joined()));
            }
            bell;
            symbol(MaterialSymbol::POWER_SETTINGS_NEW)
        }
        .gap(PITCH)
        .align_items(Align::Center),
    )
    .background(match hovered {
        true => theme().hovered(palette.background),
        false => Color::TRANSPARENT,
    })
    .radius(RADIUS)
    .padding(Inset::xy(INSET, DEPTH))
    .on_hover(|bar: &mut Bar, over: bool| bar.hover(over))
    .on_press(Bar::open)
}

fn symbol(symbol: MaterialSymbol) -> impl View {
    icon(symbol)
        .extent(Length::px(SYMBOL))
        .color(palette().text)
}
