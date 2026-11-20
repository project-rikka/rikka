use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::dither::{Dither, LEVELS, Method};
use rikka_ui::{Element, tr};

const METHODS: [Method; 2] = [Method::Ordered, Method::Diffusion];

impl Form for Dither {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let method = METHODS
            .iter()
            .position(|method| *method == tuner.current().method)
            .unwrap_or(1);
        rows::form_sheet(vec![
            tuner.amount(LEVELS),
            rows::heading(tr!("effects.dither.method")),
            tuner.segments(
                method,
                vec![
                    tr!("effects.dither.ordered"),
                    tr!("effects.dither.diffusion"),
                ],
                |effect: &mut Dither, at: usize| effect.method = METHODS[at],
            ),
        ])
    }
}
