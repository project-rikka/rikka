use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::mosaic::{CELL, Mosaic};
use rikka_ui::Element;

impl Form for Mosaic {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![tuner.amount(CELL)])
    }
}
