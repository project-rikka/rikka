use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::smart_sharpen::{
    AMOUNT, ANGLE, RADIUS, REDUCE_NOISE, Remove, SmartSharpen,
};
use rikka_ui::{Element, tr};

const REMOVALS: [Remove; 3] = [Remove::Gaussian, Remove::Lens, Remove::Motion];

impl Form for SmartSharpen {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let effect = tuner.current();
        let mut rows = vec![
            tuner.amount(AMOUNT),
            tuner.amount(RADIUS),
            tuner.amount(REDUCE_NOISE),
            tuner.choice(
                tr!("effects.smart_sharpen.remove"),
                REMOVALS.iter().position(|remove| *remove == effect.remove),
                vec![
                    tr!("effects.kind.gaussian_blur"),
                    tr!("effects.kind.lens_blur"),
                    tr!("effects.kind.motion_blur"),
                ],
                |effect: &mut SmartSharpen, at: usize| effect.remove = REMOVALS[at],
            ),
        ];
        if effect.remove == Remove::Motion {
            rows.push(tuner.angle(ANGLE, false));
        }
        rows::form_sheet(rows)
    }
}
