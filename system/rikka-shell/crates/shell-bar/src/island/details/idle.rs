use rikka_ui::{Align, Length, Space, Styled, TextAlign, TextStyled, View, palette, text, tr};
use rikka_weather::{ATTRIBUTION, TemperatureUnit};
use rikka_widgets::{column, icon, row};
use shell_feeds::degrees;

use crate::island::forecast::Forecast;
use crate::island::state::Island;

const TIME: f32 = 36.0;
const SKY: f32 = 28.0;
const TEMPERATURE: f32 = 24.0;
const LINE: f32 = 2.0;

pub(crate) fn view(island: &Island) -> impl View {
    let clock = island.clock();
    column! {
        row! {
            column! {
                text(clock.time()).text_size(TIME).light().no_wrap();
                text(clock.long_date()).muted().no_wrap();
            }
            .gap(LINE)
            .grow(1.0);
            if let Some(forecast) = island.forecast() {
                weather(forecast, island.unit());
            }
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width();
        if island.forecast().is_some() {
            text(ATTRIBUTION)
                .caption()
                .faint()
                .align(TextAlign::End)
                .no_wrap()
                .fill_width();
        }
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn weather(forecast: &Forecast, unit: TemperatureUnit) -> impl View {
    column! {
        row! {
            icon(forecast.symbol())
                .extent(Length::px(SKY))
                .filled()
                .color(palette().accent);
            text(degrees(forecast.now, unit)).text_size(TEMPERATURE).no_wrap();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center);
        text(forecast.describe()).caption().muted().no_wrap();
        if let (Some(high), Some(low)) = (forecast.high, forecast.low) {
            text(tr!(
                "shell.island.high-low",
                high = degrees(high, unit).as_str(),
                low = degrees(low, unit).as_str()
            ))
            .caption()
            .muted()
            .no_wrap();
        }
        if !forecast.place.is_empty() {
            text(forecast.place.as_str()).caption().faint().no_wrap();
        }
    }
    .gap(LINE)
    .align_items(Align::End)
}
