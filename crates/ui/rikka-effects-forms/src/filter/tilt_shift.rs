use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::tilt_shift::{ANGLE, BLUR, CENTER, FOCUS, TRANSITION, TiltShift};
use rikka_ui::{Element, tr};

impl Form for TiltShift {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            tuner.amount(BLUR),
            tuner.amount(CENTER),
            tuner.angle(ANGLE, true),
            tuner.amount(FOCUS),
            tuner.amount(TRANSITION),
            rows::note(tr!("effects.tilt_shift.note")),
        ])
    }
}
