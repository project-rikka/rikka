use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::surface_blur::{RADIUS, SurfaceBlur, THRESHOLD};
use rikka_ui::Element;

impl Form for SurfaceBlur {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![tuner.amount(RADIUS), tuner.amount(THRESHOLD)])
    }
}
