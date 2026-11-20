use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::KnobKey;
use rikka_effects::adjust::black_white::{
    BLUES, BlackWhite, CYANS, GREENS, MAGENTAS, PRESETS, REDS, YELLOWS,
};
use rikka_types::Color;
use rikka_ui::{Element, Label, tr};

const KEYS: [KnobKey<BlackWhite>; 6] = [REDS, YELLOWS, GREENS, CYANS, BLUES, MAGENTAS];

fn preset_names() -> Vec<Label> {
    vec![
        tr!("effects.black_white.default"),
        tr!("effects.black_white.red_filter"),
        tr!("effects.black_white.yellow_filter"),
        tr!("effects.black_white.infrared"),
        tr!("effects.black_white.neutral"),
    ]
}

impl Form for BlackWhite {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let effect = tuner.current();
        let weights = effect.weights();
        let mut rows = vec![tuner.preset(
            tr!("effects.black_white.preset"),
            PRESETS.iter().position(|preset| *preset == weights),
            preset_names(),
            |effect: &mut BlackWhite, at: usize| effect.set_weights(PRESETS[at]),
        )];
        rows.extend(KEYS.map(|key| tuner.amount(key)));
        rows.push(tuner.toggle(
            tr!("effects.black_white.tint"),
            effect.tinted,
            |effect: &mut BlackWhite, on: bool| effect.tinted = on,
        ));
        if effect.tinted {
            rows.push(rows::labelled(
                tr!("effects.black_white.tint_color"),
                tuner.swatch(effect.tint, |effect: &mut BlackWhite, color: Color| {
                    effect.tint = color;
                }),
            ));
        }
        rows::form_sheet(rows)
    }
}
