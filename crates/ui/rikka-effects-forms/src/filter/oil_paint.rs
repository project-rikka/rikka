use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::oil_paint::{
    ANGLE, BRISTLE_DETAIL, CLEANLINESS, OilPaint, SCALE, SHINE, STYLIZATION,
};
use rikka_ui::{Element, tr};

impl Form for OilPaint {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            rows::heading(tr!("effects.oil_paint.brush")),
            tuner.amount(STYLIZATION),
            tuner.amount(CLEANLINESS),
            tuner.amount(SCALE),
            tuner.amount(BRISTLE_DETAIL),
            rows::heading(tr!("effects.oil_paint.lighting")),
            tuner.angle(ANGLE, false),
            tuner.amount(SHINE),
        ])
    }
}
