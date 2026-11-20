use std::time::SystemTime;

use rikka_ui::{
    Align, Element, Label, Length, Space, Styled, TextAlign, TextStyled, View, palette, text, tr,
};
use rikka_weather::Hour;
use rikka_widgets::{column, column_of, row_of};
use shell_feeds::symbol;

use crate::widgets::kit::figures;
use crate::widgets::paint::trend;
use crate::widgets::weather::parts::{SMALL_TEXT, WeatherView, chance, sky, wet};

const SKY: f32 = 18.0;

#[derive(Clone, Copy)]
pub(super) struct Chart {
    pub(crate) hours: usize,
    pub(crate) trend: f32,
}

#[must_use]
pub(crate) fn hours(forecast_view: &WeatherView<'_>, chart: Chart) -> Element {
    let report = forecast_view.report;
    let coming: Vec<&Hour> = report
        .hours_from(SystemTime::now())
        .iter()
        .take(chart.hours)
        .collect();
    let degrees: Vec<f32> = coming
        .iter()
        .map(|hour| hour.temperature.celsius())
        .collect();
    let floor = degrees.iter().copied().fold(f32::INFINITY, f32::min);
    let ceiling = degrees.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let spread = (ceiling - floor).max(1.0);
    let heads = coming.iter().enumerate().map(|(index, hour)| {
        let when = match index {
            0 => tr!("shell.desk.widget.weather.now"),
            _ => tr!(
                "shell.desk.widget.weather.hour",
                hour = report.local(hour.at).hour(),
            ),
        };
        column! {
            figures(when)
                .text_size(SMALL_TEXT)
                .muted()
                .align(TextAlign::Center)
                .no_wrap()
                .fill_width();
            sky(symbol(hour.condition, hour.daytime), SKY);
            text(Label::from(forecast_view.degrees(hour.temperature)))
                .caption()
                .medium()
                .align(TextAlign::Center)
                .no_wrap()
                .fill_width();
        }
        .gap(Space::HAIR)
        .align_items(Align::Center)
        .grow(1.0)
        .min_width(0.0)
        .build()
    });
    let chances = coming
        .iter()
        .any(|hour| wet(hour.precipitation_chance).is_some())
        .then(|| {
            row_of(coming.iter().map(|hour| {
                chance(hour.precipitation_chance)
                    .align(TextAlign::Center)
                    .grow(1.0)
                    .min_width(0.0)
                    .build()
            }))
            .fill_width()
            .build()
        });
    column_of(
        [
            Some(row_of(heads).fill_width().build()),
            Some(
                trend(
                    degrees.iter().map(|degrees| (degrees - floor) / spread),
                    palette().accent,
                )
                .height(Length::px(chart.trend))
                .fill_width()
                .build(),
            ),
            chances,
        ]
        .into_iter()
        .flatten(),
    )
    .gap(Space::HAIR)
    .fill_width()
    .build()
}
