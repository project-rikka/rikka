use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::chroma_key::{ChromaKey, SOFTNESS, SPILL, TOLERANCE};
use rikka_ui::{Element, tr};

impl Form for ChromaKey {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            rows::labelled(
                tr!("effects.key.color"),
                tuner.swatch(tuner.current().color, |effect: &mut ChromaKey, color| {
                    effect.color = color;
                }),
            ),
            tuner.amount(TOLERANCE),
            tuner.amount(SOFTNESS),
            tuner.amount(SPILL),
        ])
    }
}
