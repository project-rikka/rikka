use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::motion_blur::{ANGLE, DISTANCE, MotionBlur};
use rikka_ui::Element;

impl Form for MotionBlur {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![tuner.angle(ANGLE, true), tuner.amount(DISTANCE)])
    }
}
