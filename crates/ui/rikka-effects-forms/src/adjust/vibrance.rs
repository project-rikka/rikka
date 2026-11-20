use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::adjust::vibrance::{SATURATION, VIBRANCE, Vibrance};
use rikka_ui::Element;

impl Form for Vibrance {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![tuner.amount(VIBRANCE), tuner.amount(SATURATION)])
    }
}
