use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::Parameters;
use rikka_effects::adjust::brightness_contrast::{
    BRIGHTNESS_KNOB, BrightnessContrast, CONTRAST_KNOB,
};
use rikka_ui::{Element, tr};

impl Form for BrightnessContrast {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            tuner.amount(BRIGHTNESS_KNOB),
            tuner.amount(CONTRAST_KNOB),
            tuner.toggle(
                tr!("effects.adjust.legacy"),
                tuner.current().legacy,
                |effect: &mut BrightnessContrast, legacy: bool| {
                    effect.legacy = legacy;
                    effect.normalize(None);
                },
            ),
        ])
    }
}
