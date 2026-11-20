use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::unsharp_mask::{AMOUNT, RADIUS, THRESHOLD, UnsharpMask};
use rikka_ui::Element;

impl Form for UnsharpMask {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            tuner.amount(AMOUNT),
            tuner.amount(RADIUS),
            tuner.amount(THRESHOLD),
        ])
    }
}
