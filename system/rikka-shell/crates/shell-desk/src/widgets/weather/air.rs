use rikka_ui::{Align, Element, Label, Length, Space, Styled, TextStyled, View, palette, text, tr};
use rikka_weather::{AirQuality, Report};
use rikka_widgets::{container, row, spacer};

const DOT: f32 = 7.0;

#[must_use]
pub(crate) fn air(report: &Report) -> Option<Element> {
    let index = report.air.as_ref()?.us_aqi?;
    let quality = AirQuality::of_us_aqi(index);
    let palette = palette();
    let tint = match quality {
        AirQuality::Good => palette.success,
        AirQuality::Moderate | AirQuality::UnhealthyForSensitive => palette.warning,
        AirQuality::Unhealthy | AirQuality::VeryUnhealthy | AirQuality::Hazardous => palette.danger,
    };
    let label = describe(quality).to_string();
    Some(
        row! {
            container(spacer())
                .background(tint)
                .radius(DOT / 2.0)
                .width(Length::px(DOT))
                .height(Length::px(DOT));
            text(tr!(
                "shell.desk.widget.weather.air-index",
                index = index,
                quality = label.as_str(),
            ))
            .caption()
            .muted()
            .no_wrap()
            .shrink(1.0);
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .build(),
    )
}

fn describe(quality: AirQuality) -> Label {
    let label = match quality {
        AirQuality::Good => "good",
        AirQuality::Moderate => "moderate",
        AirQuality::UnhealthyForSensitive => "sensitive",
        AirQuality::Unhealthy => "unhealthy",
        AirQuality::VeryUnhealthy => "very-unhealthy",
        AirQuality::Hazardous => "hazardous",
    };
    tr!(&format!("shell.desk.widget.weather.quality.{label}"))
}
