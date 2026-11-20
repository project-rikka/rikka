use rikka_app::prelude::*;
use rikka_keyframe::{Easing, Family};
use rikka_timeline::{CurveTile, EasingRow, curve_tile, easing_grid};

use crate::cut::Cut;

const SAMPLES: usize = 24;

pub(super) fn grid<Pick>(
    selected: Option<Easing>,
    enabled: bool,
    pick: impl Fn(Easing) -> Pick,
) -> Element
where
    Pick: Fn(&mut Cut) + 'static,
{
    easing_grid(
        Family::ALL
            .into_iter()
            .filter(|family| *family != Family::Linear)
            .map(|family| {
                EasingRow::family(family, |easing| {
                    thumbnail(easing, pick(easing))
                        .selected(selected == Some(easing))
                        .enabled(enabled)
                        .build()
                })
            }),
    )
}

pub(crate) fn thumbnail(easing: Easing, press: impl Fn(&mut Cut) + 'static) -> CurveTile {
    let samples = (0..=SAMPLES)
        .map(|step| easing.at(step as f64 / SAMPLES as f64) as f32)
        .collect();
    curve_tile(samples).on_press(press)
}
