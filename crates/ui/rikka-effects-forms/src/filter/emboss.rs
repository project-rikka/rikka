use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::emboss::{AMOUNT, ANGLE, Emboss, HEIGHT};
use rikka_ui::Element;

impl Form for Emboss {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            tuner.angle(ANGLE, false),
            tuner.amount(HEIGHT),
            tuner.amount(AMOUNT),
        ])
    }
}
