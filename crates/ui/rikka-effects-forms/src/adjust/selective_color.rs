use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::adjust::selective_color::{KEYS, SelectiveColor};
use rikka_ui::{Element, Label, Styled, local_state, tr};
use rikka_widgets::combo;

#[derive(Clone, Copy, Debug, Default)]
struct FormState {
    colors: usize,
}

fn color_names() -> Vec<Label> {
    vec![
        tr!("effects.hue_saturation.reds"),
        tr!("effects.hue_saturation.yellows"),
        tr!("effects.hue_saturation.greens"),
        tr!("effects.hue_saturation.cyans"),
        tr!("effects.hue_saturation.blues"),
        tr!("effects.hue_saturation.magentas"),
        tr!("effects.selective_color.whites"),
        tr!("effects.selective_color.neutrals"),
        tr!("effects.selective_color.blacks"),
    ]
}

impl Form for SelectiveColor {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let tuner = tuner.clone();
        local_state(FormState::default(), move |shown: FormState, show| {
            let inks = [
                tr!("effects.adjust.cyan"),
                tr!("effects.adjust.magenta"),
                tr!("effects.adjust.yellow"),
                tr!("effects.adjust.black"),
            ];
            let mut rows = vec![rows::labelled(
                tr!("effects.selective_color.colors"),
                combo(Some(shown.colors))
                    .options(color_names())
                    .on_select(move |at: usize| show(FormState { colors: at }))
                    .fill_width(),
            )];
            for (name, key) in inks.into_iter().zip(KEYS[shown.colors]) {
                rows.push(tuner.amount_as(name, key));
            }
            rows.push(rows::heading(tr!("effects.selective_color.method")));
            rows.push(tuner.segments(
                usize::from(tuner.current().absolute),
                vec![
                    tr!("effects.selective_color.relative"),
                    tr!("effects.selective_color.absolute"),
                ],
                |effect: &mut SelectiveColor, at: usize| effect.absolute = at == 1,
            ));
            rows::form_sheet(rows)
        })
    }
}
