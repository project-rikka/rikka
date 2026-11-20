use rikka_ui::{
    Align, Element, Label, Length, MaterialSymbol, Space, Styled, TextStyled, View, palette, text,
    tr,
};
use rikka_widgets::{ControlSize, button, icon, row, spacer};
use shell_feeds::percent;

use crate::island::clock::span;
use crate::island::compact::tinted;
use crate::island::live::Live;
use crate::island::notice::{Tone, sensor_name, sensor_symbol, sensor_tone, who};
use crate::island::state::Island;
use crate::state::Bar;

const SYMBOL: f32 = 20.0;

pub(crate) fn row(live: &Live, island: &Island) -> Element {
    match live {
        Live::Media(_) => Element::empty(),
        Live::Recording(_) => line(
            MaterialSymbol::FIBER_MANUAL_RECORD,
            Tone::Danger,
            tr!("shell.island.recording"),
            Label::from(span(island.elapsed())),
            Some(
                button(text(tr!("shell.island.stop")))
                    .danger()
                    .control_size(ControlSize::Small)
                    .on_press(|bar: &mut Bar| bar.stop_recording())
                    .build(),
            ),
        ),
        Live::Sensor { sensor, users } => line(
            sensor_symbol(*sensor),
            sensor_tone(*sensor),
            sensor_name(*sensor),
            who(users),
            None,
        ),
        Live::LowBattery { charge } => line(
            MaterialSymbol::BATTERY_ALERT,
            Tone::Danger,
            tr!("shell.island.battery-low"),
            Label::from(percent(*charge)),
            None,
        ),
        Live::Charging { charge, left } => {
            let detail = match left {
                Some(left) => tr!(
                    "shell.island.until-full",
                    charge = percent(*charge).as_str(),
                    time = span(*left).as_str()
                ),
                None => Label::from(percent(*charge)),
            };
            line(
                MaterialSymbol::BOLT,
                Tone::Good,
                tr!("shell.island.charging"),
                detail,
                None,
            )
        }
    }
}

fn line(
    symbol: MaterialSymbol,
    tone: Tone,
    title: Label,
    detail: Label,
    action: Option<Element>,
) -> Element {
    row! {
        icon(symbol)
            .extent(Length::px(SYMBOL))
            .filled()
            .color(tinted(tone, palette()));
        text(title).label().medium().no_wrap();
        text(detail).label().muted().no_wrap().shrink(1.0);
        spacer().grow(1.0);
        action;
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .build()
}
