use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::reduce_noise::{
    COLOR, PRESERVE_DETAILS, ReduceNoise, SHARPEN_DETAILS, STRENGTH,
};
use rikka_ui::Element;

impl Form for ReduceNoise {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            tuner.amount(STRENGTH),
            tuner.amount(PRESERVE_DETAILS),
            tuner.amount(COLOR),
            tuner.amount(SHARPEN_DETAILS),
        ])
    }
}
