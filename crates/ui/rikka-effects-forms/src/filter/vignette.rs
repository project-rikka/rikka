use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::vignette::{AMOUNT, FEATHER, HIGHLIGHTS, MIDPOINT, ROUNDNESS, Vignette};
use rikka_ui::Element;

impl Form for Vignette {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            tuner.amount(AMOUNT),
            tuner.amount(MIDPOINT),
            tuner.amount(ROUNDNESS),
            tuner.amount(FEATHER),
            tuner.amount(HIGHLIGHTS),
        ])
    }
}
