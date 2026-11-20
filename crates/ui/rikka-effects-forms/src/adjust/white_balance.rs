use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::adjust::white_balance::{TEMPERATURE, TINT, WhiteBalance};
use rikka_ui::{Element, tr};

impl Form for WhiteBalance {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            tuner.amount(TEMPERATURE),
            tuner.between(
                (tr!("effects.adjust.green"), tr!("effects.adjust.magenta")),
                TINT,
            ),
            rows::note(tr!("effects.adjust.white_balance_note")),
        ])
    }
}
