use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::box_blur::{BoxBlur, RADIUS};
use rikka_ui::Element;

impl Form for BoxBlur {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![tuner.amount(RADIUS)])
    }
}
