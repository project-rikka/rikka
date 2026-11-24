use rikka_types::Color;
use rikka_ui::{
    Align, Axis, Length, MaterialSymbol, Space, TextStyled, View, palette, text, theme,
};
use rikka_widgets::{container, icon};
use shell_feeds::{Daemons, percent, symbols};

use crate::axis;
use crate::cluster::{DEPTH, INSET, PITCH, RADIUS, SYMBOL};
use crate::state::Bar;
use crate::status::bell::Bell;

pub(crate) struct Status<'a> {
    pub(crate) daemons: &'a Daemons,
    pub(crate) bell: Option<Bell>,
    pub(crate) percentage: bool,
    pub(crate) hovered: bool,
    pub(crate) axis: Axis,
}

pub(crate) fn status(status: Status<'_>) -> impl View {
    let Status {
        daemons,
        bell,
        percentage,
        hovered,
        axis,
    } = status;
    let graph = daemons.sound().state();
    let network = daemons.network().state();
    let palette = palette();
    let battery = daemons.battery().main().map(|battery| {
        axis::line(axis, [symbol(symbols::battery(&battery)).build()])
            .extend(percentage.then(|| {
                text(percent(battery.charge))
                    .label()
                    .no_wrap()
                    .color(palette.text)
            }))
            .gap(Space::TIGHT)
            .align_items(Align::Center)
    });
    container(
        axis::line(axis, battery)
            .extend(
                graph
                    .source()
                    .map(|source| symbol(symbols::microphone(Some(&source.volume)))),
            )
            .extend(
                graph
                    .sink()
                    .map(|sink| symbol(symbols::sound(Some(&sink.volume)))),
            )
            .extend(network.radio().map(|_| {
                symbol(symbols::wireless(
                    network.is_wireless_on(),
                    network.joined(),
                ))
            }))
            .extend(bell)
            .push(symbol(MaterialSymbol::POWER_SETTINGS_NEW))
            .gap(PITCH)
            .align_items(Align::Center),
    )
    .background(match hovered {
        true => theme().hovered(palette.background),
        false => Color::TRANSPARENT,
    })
    .radius(RADIUS)
    .padding(axis::padding(axis, INSET, DEPTH))
    .on_hover(|bar: &mut Bar, over: bool| bar.hover(over))
    .on_press(Bar::open)
}

fn symbol(symbol: MaterialSymbol) -> impl View {
    icon(symbol)
        .extent(Length::px(SYMBOL))
        .color(palette().text)
}
