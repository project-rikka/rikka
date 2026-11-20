mod air;
mod days;
mod hours;
mod parts;

use desk_layout::{Size, Weather};
use rikka_ui::{Align, Element, MaterialSymbol, Space, Styled, tr};
use rikka_weather::{Spot, State, WeatherError};
use rikka_widgets::{column, row, spacer};
use shell_kit::Period;

use crate::widgets::card::padded;
use crate::widgets::kit::LINE;
use crate::widgets::quiet::quiet;
use crate::widgets::sources::Sources;
use crate::widgets::weather::hours::{Chart, hours};
use crate::widgets::weather::parts::{WeatherView, credit};

const SMALL_DEGREES: f32 = 44.0;
const SMALL_SKY: f32 = 30.0;
const MEDIUM_DEGREES: f32 = 44.0;
const MEDIUM_SIDE: f32 = 104.0;
const LARGE_DEGREES: f32 = 52.0;
const LARGE_SKY: f32 = 36.0;
const MEDIUM_CHART: Chart = Chart {
    hours: 6,
    trend: 26.0,
};
const LARGE_CHART: Chart = Chart {
    hours: 8,
    trend: 16.0,
};
const LARGE_DAYS: usize = 5;

#[must_use]
pub(crate) fn view(options: &Weather, size: Size, sources: &Sources) -> Element {
    let state = sources.weather.get();
    let unit = sources.unit.get();
    let Some(report) = state.report.as_deref() else {
        return absent(&state, size);
    };
    sources.feeds.wall.follow(Period::Minute);
    let forecast_view = WeatherView {
        report,
        place: state
            .place
            .as_ref()
            .map_or(report.place.name.as_str(), |place| place.name.as_str()),
        here: state.spot == Spot::Here,
        unit,
    };
    match size {
        Size::Small => small(&forecast_view),
        Size::Medium => medium(&forecast_view),
        Size::Large | Size::ExtraLarge => large(&forecast_view, options.air),
    }
}

fn absent(state: &State, size: Size) -> Element {
    match &state.error {
        Some(WeatherError::NoLocation) => quiet(
            MaterialSymbol::LOCATION_OFF,
            tr!("shell.desk.widget.weather.unlocated"),
        )
        .sized(size),
        Some(WeatherError::Service(_)) => quiet(
            MaterialSymbol::CLOUD_OFF,
            tr!("shell.desk.widget.weather.unavailable"),
        )
        .sized(size),
        None => quiet(
            MaterialSymbol::PARTLY_CLOUDY_DAY,
            tr!("shell.desk.widget.weather.loading"),
        )
        .sized(size),
    }
}

fn small(forecast_view: &WeatherView<'_>) -> Element {
    padded(
        column! {
            forecast_view.place();
            row! {
                forecast_view.temperature(SMALL_DEGREES);
                spacer().grow(1.0);
                forecast_view.sky(SMALL_SKY);
            }
            .align_items(Align::Center)
            .fill_width();
            forecast_view.condition();
            forecast_view.range();
            spacer().grow(1.0);
            credit(true);
        }
        .gap(LINE)
        .fill(),
    )
}

fn medium(forecast_view: &WeatherView<'_>) -> Element {
    padded(
        row! {
            column! {
                forecast_view.place();
                forecast_view.temperature(MEDIUM_DEGREES);
                forecast_view.condition();
                forecast_view.range();
                spacer().grow(1.0);
                credit(true);
            }
            .gap(LINE)
            .width(MEDIUM_SIDE)
            .fill_height();
            column! { hours(forecast_view, MEDIUM_CHART) }
                .justify(rikka_ui::Justify::Center)
                .grow(1.0)
                .min_width(0.0)
                .fill_height();
        }
        .gap(Space::NORMAL)
        .fill(),
    )
}

fn large(forecast_view: &WeatherView<'_>, air: bool) -> Element {
    let place = match air.then(|| air::air(forecast_view.report)).flatten() {
        Some(air) => forecast_view.place().beside(air),
        None => forecast_view.place(),
    };
    padded(
        column! {
            place;
            row! {
                forecast_view.temperature(LARGE_DEGREES);
                forecast_view.sky(LARGE_SKY);
                spacer().grow(1.0);
                column! {
                    forecast_view.condition();
                    forecast_view.range();
                    forecast_view.details();
                }
                .gap(LINE)
                .align_items(Align::End);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            hours(forecast_view, LARGE_CHART);
            days::days(forecast_view, LARGE_DAYS).grow(1.0);
            credit(false);
        }
        .gap(Space::TIGHT)
        .fill(),
    )
}
