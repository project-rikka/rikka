use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::adjust::posterize::{LEVELS, Posterize};
use rikka_ui::Element;

impl Form for Posterize {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![tuner.amount(LEVELS)])
    }
}
