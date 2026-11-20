use animate_cloth::Knob;
use animate_ui::{framed, labelled};
use rikka_app::prelude::*;
use rikka_physics_tailor::{ClothTuning, PieceKey};

use crate::{Desk, Dial, Studio, wording};

const MARKS: usize = 3;

#[derive(Clone, Copy)]
pub(super) enum Tuned {
    Figure,
    Piece(PieceKey),
}

impl Tuned {
    fn dial(self, knob: Knob) -> Dial {
        match self {
            Tuned::Figure => Dial::Figure(knob),
            Tuned::Piece(_) => Dial::Piece(knob),
        }
    }

    fn settle(self, studio: &mut dyn Studio, knob: Knob, value: f32) {
        match self {
            Tuned::Figure => studio.set_figure_knob(knob, value),
            Tuned::Piece(key) => studio.set_knob(key, knob, value),
        }
    }
}

pub(super) fn figure(desk: Desk<'_>) -> Option<Element> {
    let tuning = desk.figure_tuning()?;
    let reset = button(text(tr!("animate.physics.reset_tuning")))
        .small()
        .ghost()
        .focusless()
        .enabled(!tuning.is_neutral())
        .on_press(desk.act(move |studio| studio.reset_figure_tuning()));
    let mut parts = vec![
        row_of([
            text(tr!("animate.physics.whole")).bold().no_wrap().any(),
            spacer().grow(1.0).any(),
            reset.any(),
        ])
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .any(),
    ];
    parts.extend(knobs(desk, Tuned::Figure, &tuning));
    Some(framed(parts))
}

pub(super) fn knobs<'a>(
    desk: Desk<'a>,
    tuned: Tuned,
    tuning: &'a ClothTuning,
) -> impl Iterator<Item = Element> + 'a {
    Knob::ALL
        .into_iter()
        .map(move |knob| knob_row(desk, tuned, tuning, knob))
}

fn knob_row(desk: Desk<'_>, tuned: Tuned, tuning: &ClothTuning, knob: Knob) -> Element {
    let dial = tuned.dial(knob);
    let value = desk.cloth_draft(dial).unwrap_or_else(|| knob.get(tuning));
    let (least, most) = wording::knob_ends(knob);
    labelled(
        wording::knob(knob),
        column_of([
            slider(value.clamp(0.0, 1.0))
                .range(0.0..=1.0)
                .ticks(MARKS)
                .control_size(ControlSize::Small)
                .on_change(desk.act_with(move |studio, value: f32| {
                    studio.draft_cloth(dial, value);
                }))
                .on_commit(desk.act_with(move |studio, value: f32| {
                    tuned.settle(studio, knob, value);
                }))
                .fill_width()
                .any(),
            row_of([
                text(least).caption().muted().no_wrap().any(),
                spacer().grow(1.0).any(),
                text(most).caption().muted().no_wrap().any(),
            ])
            .fill_width()
            .any(),
        ])
        .gap(Space::NONE)
        .fill_width()
        .any(),
        None,
    )
}
