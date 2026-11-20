use crate::editor::HueBars;
use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::adjust::hue_saturation::{HueSaturation, KEYS};
use rikka_ui::{Element, Label, Styled, View, local_state, tr};
use rikka_widgets::combo;

#[derive(Clone, Copy, Debug, Default)]
struct FormState {
    range: usize,
}

fn range_names() -> Vec<Label> {
    vec![
        tr!("effects.hue_saturation.master"),
        tr!("effects.hue_saturation.reds"),
        tr!("effects.hue_saturation.yellows"),
        tr!("effects.hue_saturation.greens"),
        tr!("effects.hue_saturation.cyans"),
        tr!("effects.hue_saturation.blues"),
        tr!("effects.hue_saturation.magentas"),
    ]
}

impl Form for HueSaturation {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let tuner = tuner.clone();
        local_state(FormState::default(), move |shown: FormState, show| {
            let effect = tuner.current();
            let range = match effect.colorize {
                true => 7,
                false => shown.range,
            };
            let [hue, saturation, lightness] = KEYS[range];
            let mut rows = Vec::new();
            if !effect.colorize {
                rows.push(rows::labelled(
                    tr!("effects.hue_saturation.edit"),
                    combo(Some(shown.range))
                        .options(range_names())
                        .on_select(move |at: usize| show(FormState { range: at }))
                        .fill_width(),
                ));
            }
            rows.extend([
                tuner.amount_as(tr!("effects.adjust.hue"), hue),
                tuner.amount_as(tr!("effects.adjust.saturation"), saturation),
                tuner.amount_as(tr!("effects.adjust.lightness"), lightness),
            ]);
            if !effect.colorize {
                let around = (shown.range > 0).then(|| (shown.range - 1) as f32 * 60.0);
                rows.push(HueBars::new(tuner.value(hue)).centered_on(around).any());
            }
            rows.push(tuner.toggle(
                tr!("effects.hue_saturation.colorize"),
                effect.colorize,
                |effect: &mut HueSaturation, on: bool| effect.colorize = on,
            ));
            rows::form_sheet(rows)
        })
    }
}
