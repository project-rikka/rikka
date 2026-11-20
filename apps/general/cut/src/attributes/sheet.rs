use cut_project::{Attribute, KeyFit};
use rikka_app::prelude::*;
use rikka_widgets::{CheckState, column};

use super::Pasting;
use crate::cut::Cut;
use crate::view::STAGE;
use crate::wording;

const WIDTH: f32 = 400.0;
const LIST: f32 = 240.0;
const FITS: [KeyFit; 2] = [KeyFit::Maintain, KeyFit::Stretch];

pub(crate) fn sheet(pasting: &Pasting) -> Vec<Element> {
    let effects: Vec<Attribute> = pasting.effects().collect();
    let others: Vec<Attribute> = pasting.others().collect();
    let picked_effects = effects
        .iter()
        .filter(|attribute| pasting.picked.contains(attribute))
        .count();
    let every = match picked_effects {
        0 => CheckState::Unchecked,
        count if count == effects.len() => CheckState::Checked,
        _ => CheckState::Indeterminate,
    };
    let choice = |attribute: Attribute| {
        checkbox(pasting.picked.contains(&attribute))
            .control_size(ControlSize::Small)
            .label(text(wording::attribute(attribute, &pasting.from)))
            .on_toggle(move |cut: &mut Cut, on: bool| {
                cut.tune_pasting(|pasting| pasting.pick(attribute, on));
            })
            .build()
    };
    let effect_rows: Vec<Element> = effects.iter().copied().map(choice).collect();
    let other_rows: Vec<Element> = others.iter().copied().map(choice).collect();
    let fit = FITS
        .iter()
        .position(|keys| *keys == pasting.keys)
        .unwrap_or(0);
    let body = column! {
        scrollable(
            column! {
                if !effects.is_empty() {
                    checkbox(every)
                        .control_size(ControlSize::Small)
                        .label(text(tr!("cut.panel.effects")).semibold())
                        .on_toggle(|cut: &mut Cut, on: bool| {
                            cut.tune_pasting(|pasting| pasting.pick_effects(on));
                        });
                    column_of(effect_rows)
                        .gap(Space::SNUG)
                        .padding(Inset::edges(Space::NONE, Space::NONE, Space::NONE, Space::ROOMY))
                        .fill_width();
                }
                column_of(other_rows).gap(Space::SNUG).fill_width();
            }
            .gap(Space::NORMAL)
            .fill_width(),
        )
        .max_height(LIST)
        .fill_width();
        field(
            tr!("cut.attributes.keys"),
            segmented(fit)
                .segments(FITS.map(wording::key_fit))
                .control_size(ControlSize::Small)
                .on_select(|cut: &mut Cut, at: usize| {
                    cut.tune_pasting(|pasting| pasting.keys = FITS[at]);
                })
                .fill_width(),
        );
    }
    .gap(Space::ROOMY)
    .fill_width();
    dialog(tr!("cut.attributes.title"), body)
        .summary(tr!("cut.attributes.onto", count = pasting.onto.len()))
        .cancel(tr!("cut.attributes.cancel"), Cut::close_pasting)
        .accept(tr!("cut.attributes.paste"), Cut::paste_picked)
        .ready(!pasting.picked.is_empty())
        .width(WIDTH)
        .keyed("cut-attributes")
        .centered_on(STAGE)
        .with_scrim()
}
