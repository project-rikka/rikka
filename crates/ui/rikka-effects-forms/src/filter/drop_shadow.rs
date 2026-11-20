use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::drop_shadow::{ANGLE, BLUR, DISTANCE, DropShadow, OPACITY};
use rikka_ui::{Element, tr};

impl Form for DropShadow {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            rows::labelled(
                tr!("effects.drop_shadow.color"),
                tuner.swatch(tuner.current().color, |effect: &mut DropShadow, color| {
                    effect.color = color;
                }),
            ),
            tuner.angle(ANGLE, false),
            tuner.amount(DISTANCE),
            tuner.amount(BLUR),
            tuner.amount(OPACITY),
        ])
    }
}
