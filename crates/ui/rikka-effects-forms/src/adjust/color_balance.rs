use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::adjust::color_balance::{ColorBalance, KEYS, axes};
use rikka_ui::{Element, Styled, View, local_state, tr};
use rikka_widgets::segmented;

#[derive(Clone, Copy, Debug)]
struct FormState {
    tone: usize,
}

const MIDTONES: usize = 1;

impl Form for ColorBalance {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let tuner = tuner.clone();
        local_state(
            FormState { tone: MIDTONES },
            move |shown: FormState, show| {
                let mut rows = vec![
                    rows::heading(tr!("effects.color_balance.tone")),
                    segmented(shown.tone)
                        .segments(vec![
                            tr!("effects.adjust.shadows"),
                            tr!("effects.adjust.midtones"),
                            tr!("effects.adjust.highlights"),
                        ])
                        .on_select(move |at: usize| show(FormState { tone: at }))
                        .fill_width()
                        .any(),
                ];
                for (ends, key) in axes().into_iter().zip(KEYS[shown.tone]) {
                    rows.push(tuner.between(ends, key));
                }
                rows.push(tuner.toggle(
                    tr!("effects.adjust.preserve_luminosity"),
                    tuner.current().preserve_luminosity,
                    |effect: &mut ColorBalance, on: bool| effect.preserve_luminosity = on,
                ));
                rows::form_sheet(rows)
            },
        )
    }
}
