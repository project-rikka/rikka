use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::luma_key::{Keep, LumaKey, SOFTNESS, THRESHOLD};
use rikka_ui::{Element, tr};

impl Form for LumaKey {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            tuner.amount(THRESHOLD),
            tuner.amount(SOFTNESS),
            rows::heading(tr!("effects.key.keep")),
            tuner.segments(
                usize::from(tuner.current().keeps == Keep::Dark),
                vec![tr!("effects.key.bright"), tr!("effects.key.dark")],
                |effect: &mut LumaKey, at: usize| {
                    effect.keeps = match at {
                        1 => Keep::Dark,
                        _ => Keep::Bright,
                    };
                },
            ),
        ])
    }
}
