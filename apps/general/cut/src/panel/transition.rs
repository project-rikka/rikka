use cut_project::{
    Direction, EditPoint, Project, Transition, TransitionAlignment, TransitionKind, TransitionKnob,
};
use rikka_app::prelude::*;
use rikka_effects::{Scale, Unit};
use rikka_widgets::row;

use super::animation_presets::{POINTS, arrow};
use super::rows::{Apply, easing_row, group_with, picker_row, slider_row, toggle_row};
use crate::controls::{glyph_button, toggle};
use crate::cut::Cut;
use crate::wording;

const EASING: &str = "cut-transition-easing";
const GLYPH: f32 = 16.0;
const COMPASS: f32 = 96.0;
const ALIGNMENT: f32 = 240.0;
const NONE_SELECTED: usize = usize::MAX;

fn tuned<T: Copy + 'static>(
    set: impl Fn(&mut Transition, T) + Clone + 'static,
) -> (impl Apply<T>, impl Apply<T>) {
    let previewed = set.clone();
    (
        move |cut: &mut Cut, value: T| {
            cut.preview_transition(|transition| previewed(transition, value))
        },
        move |cut: &mut Cut, value: T| cut.retune_transition(|transition| set(transition, value)),
    )
}

fn retuned<T: Copy + 'static>(set: impl Fn(&mut Transition, T) + Clone + 'static) -> impl Apply<T> {
    move |cut: &mut Cut, value: T| cut.retune_transition(|transition| set(transition, value))
}

pub(crate) fn settings(project: &Project, edit_point: EditPoint) -> Option<Element> {
    let transition_span = project.transition_span(edit_point)?;
    let transition = *transition_span.transition;
    let kind = transition.kind;
    let room = transition_span.out.map_or(0, |clip| clip.length())
        + transition_span.into.map_or(0, |clip| clip.length());
    let mut rows = vec![kinds(kind), length(transition.length(), room.max(2))];
    if transition_span.out.is_some() && transition_span.into.is_some() {
        rows.push(alignment(transition.alignment_at_cut()));
    }
    rows.push(easing_row(
        tr!("cut.transition.easing"),
        (EASING, edit_point),
        transition.easing,
        |cut: &mut Cut, anchor: Anchor| cut.open_transition_easing(anchor),
    ));
    if !kind.directions().is_empty() {
        rows.push(compass(kind, transition.direction));
    }
    rows.extend(kind.knobs().iter().map(|knob| knob_row(&transition, *knob)));
    let sounding = [transition_span.out, transition_span.into]
        .into_iter()
        .flatten()
        .any(|clip| project.has_sound(clip));
    if sounding {
        rows.push(toggle_row(
            tr!("cut.transition.sound"),
            transition.sound,
            retuned(|transition: &mut Transition, on: bool| transition.sound = on),
        ));
    }
    let remove = glyph_button(MaterialSymbol::DELETE, GLYPH)
        .ghost()
        .on_press(Cut::remove_transition)
        .tooltip(tr!("cut.transition.remove"));
    Some(
        group_with(tr!("cut.transition.group"), Some(remove), rows)
            .build()
            .keyed(("cut-transition", edit_point)),
    )
}

fn kinds(kind: TransitionKind) -> Element {
    picker_row(
        tr!("cut.transition.kind"),
        Some(kind.index()),
        TransitionKind::ALL.map(wording::transition_kind).to_vec(),
        retuned(|transition: &mut Transition, at: usize| {
            if let Some(kind) = TransitionKind::from_index(at) {
                *transition = transition.with_kind(kind);
            }
        }),
    )
}

fn length(frames: i64, room: i64) -> Element {
    slider_row(
        tr!("cut.transition.length"),
        frames as f32,
        Scale::new(1.0..=room as f32).unit(Unit::Frames),
        tuned(|transition: &mut Transition, frames: f32| {
            *transition = transition.with_length(frames.round().max(1.0) as i64);
        }),
        None,
    )
}

fn alignment(at: Option<TransitionAlignment>) -> Element {
    let selected = at
        .and_then(|at| {
            TransitionAlignment::ALL
                .iter()
                .position(|candidate| *candidate == at)
        })
        .unwrap_or(NONE_SELECTED);
    let commit = retuned(|transition: &mut Transition, at: usize| {
        if let Some(at) = TransitionAlignment::ALL.get(at) {
            *transition = transition.with_alignment(*at);
        }
    });
    field(
        tr!("cut.transition.at_cut"),
        segmented(selected)
            .segments(TransitionAlignment::ALL.map(wording::alignment_at_cut))
            .control_size(ControlSize::Small)
            .focusless()
            .on_select(commit)
            .width(ALIGNMENT),
    )
    .build()
}

fn compass(kind: TransitionKind, selected: Direction) -> Element {
    let allowed = kind.directions();
    let cells = POINTS.map(
        |point| match point.filter(|direction| allowed.contains(direction)) {
            Some(direction) => toggle(
                arrow(direction),
                GLYPH,
                direction == selected,
                move |cut: &mut Cut| {
                    cut.retune_transition(|transition| transition.direction = direction)
                },
            )
            .build(),
            None => spacer().build(),
        },
    );
    field(
        tr!("cut.transition.direction"),
        grid_of(3, cells).width(COMPASS),
    )
    .build()
}

fn knob_row(transition: &Transition, knob: TransitionKnob) -> Element {
    let kind = transition.kind;
    let label = wording::transition_knob(kind, knob);
    let Some(scale) = knob.scale(kind) else {
        let (preview, commit) = tuned(|transition: &mut Transition, tint: Color| {
            transition.tuning = transition.tuning.tinted(tint);
        });
        return field(
            label,
            row! {
                color_well(transition.tuning.tint)
                    .control_size(ControlSize::Small)
                    .on_change(preview)
                    .on_commit(commit);
            },
        )
        .build();
    };
    let value = transition
        .tuning
        .value(knob)
        .unwrap_or(*scale.range.start());
    slider_row(
        label,
        value,
        scale,
        tuned(move |transition: &mut Transition, value: f32| {
            transition.tuning = transition.tuning.with(transition.kind, knob, value);
        }),
        None,
    )
}
