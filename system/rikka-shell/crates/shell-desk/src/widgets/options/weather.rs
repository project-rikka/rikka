use std::rc::Rc;

use desk_layout::{Content, Weather};
use rikka_session::settings_app::PROGRAM;
use rikka_ui::{Element, Label, Space, Styled, TextStyled, View, text, tr};
use rikka_weather::{Spot, State};
use rikka_widgets::{ControlSize, button, column, field};

use crate::widgets::live::Live;
use crate::widgets::options::{Change, toggle};
use crate::widgets::sources::Sources;

#[must_use]
pub(crate) fn options(weather: &Weather, sources: &Sources, change: &Change) -> Element {
    let change = Rc::clone(change);
    let place = sources.weather.with(placed);
    column! {
        toggle(
            tr!("shell.desk.widget.options.air"),
            weather.air,
            move |air| change(Content::Weather(Weather { air })),
        );
        field(
            tr!("shell.desk.widget.options.place"),
            text(place).muted()
                .no_wrap()
                .shrink(1.0),
        );
        field(
            tr!("shell.desk.widget.options.place-set"),
            button(text(tr!("shell.desk.widget.options.open-settings")))
                .secondary()
                .control_size(ControlSize::Small)
                .on_press(|live: &mut Live| live.desk_scene().launch([PROGRAM])),
        );
    }
    .gap(Space::NORMAL)
    .fill_width()
    .build()
}

fn placed(state: &State) -> Label {
    let named = state
        .place
        .as_ref()
        .or_else(|| state.report.as_ref().map(|report| &report.place))
        .map(|place| match (&place.region, &place.country) {
            (_, Some(country)) => format!("{}, {country}", place.name),
            (Some(region), None) => format!("{}, {region}", place.name),
            (None, None) => place.name.clone(),
        });
    match (named, &state.spot) {
        (Some(named), Spot::Here) => tr!(
            "shell.desk.widget.options.place-here",
            place = named.as_str()
        ),
        (Some(named), Spot::Fixed(_)) => Label::from(named),
        (None, Spot::Fixed(place)) => Label::from(place.name.as_str()),
        (None, Spot::Here) => tr!("shell.desk.widget.weather.loading"),
    }
}
