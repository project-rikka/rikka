use std::ops::RangeInclusive;

use rikka_app::prelude::*;
use rikka_settings::session::appearance;
use rikka_widgets::column;

use crate::amount::{amount, dimmed_amount};
use crate::appearance::component::Appearance;
use crate::controls::{Choices, choice, flag_or};
use crate::domain::DomainPage;

const BACKGROUNDS: Choices = &[
    (
        appearance::BACKGROUND_SOLID,
        "settings.appearance.glass.solid",
    ),
    (
        appearance::BACKGROUND_GLASS,
        "settings.appearance.glass.glass",
    ),
    (
        appearance::BACKGROUND_LIQUID,
        "settings.appearance.glass.liquid",
    ),
];

const OPACITY: RangeInclusive<f32> = 0.0..=1.0;
const OPACITY_STEP: f32 = 0.05;

const REACH_STEP: f32 = 4.0;

pub(crate) fn glass(appearance: &Appearance) -> impl View {
    let dimmed = !blurs(appearance);
    section(
        tr!("settings.appearance.glass.title"),
        column! {
            choice(
                appearance,
                tr!("settings.appearance.glass.background"),
                appearance::BACKGROUND,
                BACKGROUNDS,
            );
            flag_or(
                appearance,
                tr!("settings.appearance.glass.blur"),
                appearance::BLUR,
                appearance::BLUR_DEFAULT,
            );
            reach_row(appearance, dimmed);
            opacity_row(appearance);
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn reach_row(appearance: &Appearance, dimmed: bool) -> impl View {
    let reach = number(
        appearance,
        appearance::BLUR_RADIUS,
        appearance::BLUR_RADIUS_DEFAULT,
    );
    field(
        tr!("settings.appearance.glass.reach"),
        dimmed_amount(
            reach,
            0.0..=appearance::BLUR_RADIUS_MAX as f32,
            REACH_STEP,
            format!("{reach:.0} px"),
            |appearance: &mut Appearance, reach: f32| {
                appearance
                    .domain_mut()
                    .set(appearance::BLUR_RADIUS, f64::from(reach));
            },
            dimmed,
        ),
    )
    .dimmed(dimmed)
}

fn opacity_row(appearance: &Appearance) -> impl View {
    let opacity = number(appearance, appearance::OPACITY, appearance::OPACITY_DEFAULT);
    field(
        tr!("settings.appearance.glass.opacity"),
        amount(
            opacity,
            OPACITY,
            OPACITY_STEP,
            format!("{:.0}%", opacity * 100.0),
            |appearance: &mut Appearance, opacity: f32| {
                appearance
                    .domain_mut()
                    .set(appearance::OPACITY, f64::from(opacity));
            },
        ),
    )
}

fn blurs(appearance: &Appearance) -> bool {
    appearance
        .domain()
        .get(appearance::BLUR)
        .unwrap_or(appearance::BLUR_DEFAULT)
}

fn number(appearance: &Appearance, key: &str, absent: f64) -> f32 {
    appearance.domain().get(key).unwrap_or(absent) as f32
}
