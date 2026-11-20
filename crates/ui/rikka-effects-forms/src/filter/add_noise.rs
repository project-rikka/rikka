use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::add_noise::{AMOUNT, AddNoise, SEED, Spread};
use rikka_ui::{Element, tr};

const SPREADS: [Spread; 2] = [Spread::Uniform, Spread::Gaussian];

impl Form for AddNoise {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let effect = tuner.current();
        let distribution = SPREADS
            .iter()
            .position(|spread| *spread == effect.distribution)
            .unwrap_or(0);
        rows::form_sheet(vec![
            tuner.amount(AMOUNT),
            rows::heading(tr!("effects.add_noise.distribution")),
            tuner.segments(
                distribution,
                vec![
                    tr!("effects.add_noise.uniform"),
                    tr!("effects.add_noise.gaussian"),
                ],
                |effect: &mut AddNoise, at: usize| effect.distribution = SPREADS[at],
            ),
            tuner.toggle(
                tr!("effects.add_noise.monochromatic"),
                effect.monochromatic,
                |effect: &mut AddNoise, on: bool| effect.monochromatic = on,
            ),
            tuner.fielded(tr!("effects.add_noise.seed"), SEED),
        ])
    }
}
