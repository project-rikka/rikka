use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::high_pass::{HighPass, RADIUS};
use rikka_ui::Element;

impl Form for HighPass {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![tuner.amount(RADIUS)])
    }
}
