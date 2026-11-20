use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::clarity::{AMOUNT, Clarity, RADIUS};
use rikka_ui::Element;

impl Form for Clarity {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![tuner.amount(AMOUNT), tuner.amount(RADIUS)])
    }
}
