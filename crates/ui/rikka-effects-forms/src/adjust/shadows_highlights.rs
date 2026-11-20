use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::adjust::shadows_highlights::{
    COLOR, HIGHLIGHT_TONE, HIGHLIGHTS, MIDTONES, RADIUS, SHADOW_TONE, SHADOWS, ShadowsHighlights,
};
use rikka_ui::{Element, tr};

impl Form for ShadowsHighlights {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![
            rows::heading(tr!("effects.adjust.shadows")),
            tuner.amount_as(tr!("effects.adjust.amount"), SHADOWS),
            tuner.amount_as(tr!("effects.shadows_highlights.tone"), SHADOW_TONE),
            rows::heading(tr!("effects.adjust.highlights")),
            tuner.amount_as(tr!("effects.adjust.amount"), HIGHLIGHTS),
            tuner.amount_as(tr!("effects.shadows_highlights.tone"), HIGHLIGHT_TONE),
            rows::heading(tr!("effects.shadows_highlights.adjustments")),
            tuner.amount(RADIUS),
            tuner.amount(COLOR),
            tuner.amount(MIDTONES),
        ])
    }
}
