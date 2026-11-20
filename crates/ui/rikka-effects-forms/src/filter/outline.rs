use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::outline::{Outline, SOFTNESS, WIDTH_KNOB};
use rikka_ui::{Element, tr};

impl Form for Outline {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            rows::labelled(
                tr!("effects.outline.color"),
                tuner.swatch(tuner.current().color, |effect: &mut Outline, color| {
                    effect.color = color;
                }),
            ),
            tuner.amount(WIDTH_KNOB),
            tuner.amount(SOFTNESS),
        ])
    }
}
