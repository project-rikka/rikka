use std::time::SystemTime;

use rikka_ui::{
    Align, Element, Label, Length, Space, Styled, TextAlign, TextStyled, View, palette, text, tr,
};
use rikka_weather::Day;
use rikka_widgets::{column_of, row};
use shell_feeds::{NameLength, symbol, weekday};

use crate::widgets::paint::span;
use crate::widgets::weather::parts::{WeatherView, chance, sky};

const NAME: f32 = 46.0;
const CHANCE: f32 = 30.0;
const DEGREES: f32 = 32.0;
const SKY: f32 = 16.0;
const BAR: f32 = 4.0;

#[must_use]
pub(crate) fn days(forecast_view: &WeatherView<'_>, shown: usize) -> Element {
    let report = forecast_view.report;
    let today = report.local(SystemTime::now()).date();
    let days: Vec<&Day> = report
        .days
        .iter()
        .filter(|day| day.date >= today)
        .take(shown)
        .collect();
    let floor = days
        .iter()
        .map(|day| day.low.celsius())
        .fold(forecast_view.now().temperature.celsius(), f32::min);
    let ceiling = days
        .iter()
        .map(|day| day.high.celsius())
        .fold(forecast_view.now().temperature.celsius(), f32::max);
    let spread = (ceiling - floor).max(1.0);
    let place = move |degrees: f32| (degrees - floor) / spread;
    let now = place(forecast_view.now().temperature.celsius());
    column_of(days.into_iter().enumerate().map(|(index, day)| {
        let name = match index {
            0 => tr!("shell.desk.widget.weather.today"),
            _ => weekday(day.date.weekday(), NameLength::Short),
        };
        let range = span(
            place(day.low.celsius()),
            place(day.high.celsius()),
            palette().accent,
        );
        let range = match index {
            0 => range.marked(now),
            _ => range,
        };
        row! {
            text(name).label().no_wrap().width(Length::px(NAME));
            sky(symbol(day.condition, true), SKY);
            chance(day.precipitation_chance)
                .align(TextAlign::End)
                .width(Length::px(CHANCE));
            text(Label::from(forecast_view.degrees(day.low)))
                .label()
                .muted()
                .align(TextAlign::End)
                .no_wrap()
                .width(Length::px(DEGREES));
            range.height(Length::px(BAR)).grow(1.0);
            text(Label::from(forecast_view.degrees(day.high)))
                .label()
                .medium()
                .align(TextAlign::End)
                .no_wrap()
                .width(Length::px(DEGREES));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .grow(1.0)
        .fill_width()
        .build()
    }))
    .fill_width()
    .build()
}
