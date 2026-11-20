use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::adjust::exposure::{EXPOSURE, Exposure, GAMMA, OFFSET};
use rikka_ui::Element;

impl Form for Exposure {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            tuner.amount(EXPOSURE),
            tuner.amount(OFFSET),
            tuner.amount(GAMMA),
        ])
    }
}
