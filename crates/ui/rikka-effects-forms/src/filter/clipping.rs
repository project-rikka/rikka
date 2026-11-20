use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::clipping::{BOTTOM, Clipping, FEATHER, LEFT, RIGHT, TOP};
use rikka_ui::Element;

impl Form for Clipping {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            tuner.amount(LEFT),
            tuner.amount(TOP),
            tuner.amount(RIGHT),
            tuner.amount(BOTTOM),
            tuner.amount(FEATHER),
        ])
    }
}
