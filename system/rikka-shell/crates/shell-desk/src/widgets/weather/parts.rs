use rikka_ui::{
    Align, Element, Label, Length, MaterialSymbol, Space, Styled, Text, TextStyled, View, palette,
    text, tr,
};
use rikka_weather::{ATTRIBUTION, Day, Now, Report, Temperature, TemperatureUnit};
use rikka_widgets::{icon, row, row_of};
use shell_feeds::{degrees, describe, percent, symbol};

use crate::widgets::kit::{Header, display, figures, header};

pub(super) const SMALL_TEXT: f32 = 10.0;

const CREDIT: f32 = 9.0;
const DETAIL: f32 = 12.0;
const DETAIL_GAP: f32 = 3.0;
const WET: f32 = 0.2;

pub(crate) struct WeatherView<'a> {
    pub(crate) report: &'a Report,
    pub(crate) place: &'a str,
    pub(crate) here: bool,
    pub(crate) unit: TemperatureUnit,
}

impl WeatherView<'_> {
    pub(crate) fn now(&self) -> &Now {
        &self.report.now
    }

    pub(crate) fn today(&self) -> Option<&Day> {
        self.report.today()
    }

    pub(crate) fn degrees(&self, temperature: Temperature) -> String {
        degrees(temperature, self.unit)
    }

    pub(crate) fn place(&self) -> Header {
        header(
            self.here.then_some(MaterialSymbol::NEAR_ME),
            Label::from(self.place),
        )
    }

    pub(crate) fn temperature(&self, size: f32) -> Element {
        display(self.degrees(self.now().temperature), size).build()
    }

    pub(crate) fn sky(&self, size: f32) -> Element {
        let now = self.now();
        sky(symbol(now.condition, now.daytime), size)
    }

    pub(crate) fn condition(&self) -> Element {
        text(describe(self.now().condition))
            .label()
            .medium()
            .no_wrap()
            .build()
    }

    pub(crate) fn range(&self) -> Option<Element> {
        let today = self.today()?;
        Some(
            text(tr!(
                "shell.desk.widget.weather.high-low",
                high = self.degrees(today.high).as_str(),
                low = self.degrees(today.low).as_str(),
            ))
            .caption()
            .muted()
            .no_wrap()
            .build(),
        )
    }

    pub(crate) fn details(&self) -> Element {
        let now = self.now();
        let chips = [
            now.feels_like
                .map(|feels| (Some(MaterialSymbol::THERMOSTAT), self.degrees(feels))),
            now.humidity
                .map(|humidity| (Some(MaterialSymbol::WATER_DROP), percent(humidity))),
            now.uv_index.map(|uv| (None, format!("UV {uv:.0}"))),
        ]
        .into_iter()
        .flatten()
        .map(|(symbol, shown_text)| {
            row! {
                if let Some(symbol) = symbol {
                    icon(symbol)
                        .extent(Length::px(DETAIL))
                        .color(palette().text_muted);
                }
                text(Label::from(shown_text)).caption().muted().no_wrap();
            }
            .gap(DETAIL_GAP)
            .align_items(Align::Center)
            .build()
        });
        row_of(chips)
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .build()
    }
}

#[must_use]
pub(crate) fn sky(symbol: MaterialSymbol, size: f32) -> Element {
    icon(symbol)
        .extent(Length::px(size))
        .filled()
        .color(palette().accent)
        .build()
}

#[must_use]
pub(super) fn wet(chance: Option<f32>) -> Option<String> {
    chance.filter(|chance| *chance >= WET).map(percent)
}

#[must_use]
pub(super) fn chance(chance: Option<f32>) -> Text {
    figures(Label::from(wet(chance).unwrap_or_default()))
        .text_size(SMALL_TEXT)
        .color(palette().accent)
        .no_wrap()
}

#[must_use]
pub(super) fn credit(wraps: bool) -> Element {
    let credit = text(ATTRIBUTION).text_size(CREDIT).faint();
    match wraps {
        true => credit.fill_width().build(),
        false => credit.no_wrap().build(),
    }
}
