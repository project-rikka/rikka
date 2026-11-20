use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::glow::{BRIGHTNESS, Glow, RADIUS, THRESHOLD};
use rikka_ui::Element;

impl Form for Glow {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            tuner.amount(RADIUS),
            tuner.amount(BRIGHTNESS),
            tuner.amount(THRESHOLD),
        ])
    }
}
