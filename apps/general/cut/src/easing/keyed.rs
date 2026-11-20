use cut_project::{Clip, ClipId, Interpolation, KeyRef, Side, Smooth};
use rikka_app::prelude::*;
use rikka_ui::Action;
use rikka_widgets::{column, row};

use super::graph::SegmentGraph;
use super::segment::{HandleChange, Segment};
use super::{curves, held_keys};
use crate::cut::Cut;
use crate::wording;

const SMOOTHS: [Smooth; 3] = [Smooth::Manual, Smooth::Continuous, Smooth::Auto];
const COLUMN: f32 = 252.0;
const FIELD: f32 = 84.0;
const SMOOTHING: f32 = 156.0;
const MIN_INFLUENCE: f32 = 0.1;

pub(super) fn keyed(clip: &Clip, keys: &[KeyRef], per_second: f64) -> Element {
    let id = clip.id();
    let selected_keys = held_keys(clip, keys);
    let first = selected_keys.first().cloned();
    let timing = first
        .as_ref()
        .and_then(|key| clip.channel(&key.property)?.timing(key.at - clip.start()));
    let current = first.as_ref().and_then(|key| clip.interpolation(key));
    let preset = match current {
        Some(Interpolation::Preset(easing)) => Some(easing),
        _ => None,
    };
    let picked = selected_keys.clone();
    let grid = curves::grid(preset, !selected_keys.is_empty(), move |easing| {
        let keys = picked.clone();
        move |cut: &mut Cut| cut.ease_keys(id, &keys, Interpolation::Preset(easing))
    });
    let editor = first
        .as_ref()
        .and_then(|key| Some(editor(id, key, &Segment::for_key(clip, key)?, per_second)));
    let smoothing = first
        .zip(timing)
        .map(|(key, timing)| smoothing(id, key, timing.smooth));
    row! {
        column! {
            text(tr!("cut.easing.curves")).label().semibold();
            grid;
        }
        .gap(Space::SNUG);
        column! {
            text(tr!("cut.key.ease")).label().semibold();
            basics(id, &selected_keys, current);
            if let Some(editor) = editor {
                editor;
            }
            if let Some(smoothing) = smoothing {
                smoothing;
            }
            actions(clip, keys, &selected_keys);
        }
        .gap(Space::SNUG)
        .width(COLUMN);
    }
    .gap(Space::ROOMY)
    .build()
}

fn basics(clip: ClipId, selected_keys: &[KeyRef], current: Option<Interpolation>) -> Element {
    let choices = Interpolation::PLAIN
        .into_iter()
        .chain(Interpolation::EASY)
        .map(|interpolation| {
            let keys = selected_keys.to_vec();
            let selected = button(text(wording::interpolation(interpolation)).no_wrap())
                .small()
                .focusless()
                .enabled(!selected_keys.is_empty())
                .on_press(move |cut: &mut Cut| cut.ease_keys(clip, &keys, interpolation));
            match current == Some(interpolation) {
                true => selected.secondary(),
                false => selected.ghost(),
            }
            .fill_width()
        });
    grid_of(2, choices).gap(Space::TIGHT).fill_width().build()
}

fn editor(clip: ClipId, key: &KeyRef, segment: &Segment, per_second: f64) -> Element {
    let (dragged, released) = (key.clone(), key.clone());
    let graph = SegmentGraph {
        curve: segment.curve().to_vec(),
        handles: segment.handles(),
        end: segment.end(),
        on_drag: Action::new(move |cut: &mut Cut, side: Side, direction: Point| {
            cut.bend_segment(clip, &dragged, side, direction, false);
        }),
        on_release: Action::new(move |cut: &mut Cut, side: Side, direction: Point| {
            cut.bend_segment(clip, &released, side, direction, true);
        }),
    };
    let sides = [Side::Outgoing, Side::Incoming];
    let mut heading = vec![spacer().grow(1.0).build()];
    heading.extend(
        [tr!("cut.easing.start"), tr!("cut.easing.end")]
            .map(|label| text(label).caption().muted().no_wrap().width(FIELD).build()),
    );
    let mut speeds = vec![text(tr!("cut.easing.speed")).no_wrap().grow(1.0).build()];
    speeds.extend(sides.map(|side| speed(clip, key, segment, side, per_second)));
    let mut influences = vec![
        text(tr!("cut.easing.influence"))
            .no_wrap()
            .grow(1.0)
            .build(),
    ];
    influences.extend(sides.map(|side| influence(clip, key, segment, side)));
    let lines = [heading, speeds, influences].map(|cells| {
        row_of(cells)
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .fill_width()
            .build()
    });
    let mut parts = vec![graph.build()];
    parts.extend(lines);
    column_of(parts).gap(Space::TIGHT).fill_width().build()
}

fn speed(clip: ClipId, key: &KeyRef, segment: &Segment, side: Side, per_second: f64) -> Element {
    let key = key.clone();
    let handle_speed = f64::from(segment.handle(side).speed) * per_second;
    number_field(handle_speed as f32)
        .decimals(2)
        .unit(tr!("cut.easing.per_second"))
        .control_size(ControlSize::Small)
        .on_commit(move |cut: &mut Cut, speed: f32| {
            let speed = (f64::from(speed) / per_second) as f32;
            cut.set_segment_handle(clip, &key, side, HandleChange::Speed(speed));
        })
        .width(FIELD)
        .build()
}

fn influence(clip: ClipId, key: &KeyRef, segment: &Segment, side: Side) -> Element {
    let key = key.clone();
    number_field(segment.handle(side).influence * 100.0)
        .range(MIN_INFLUENCE..=100.0)
        .decimals(1)
        .unit(tr!("cut.unit.percent"))
        .control_size(ControlSize::Small)
        .on_commit(move |cut: &mut Cut, influence: f32| {
            cut.set_segment_handle(clip, &key, side, HandleChange::Influence(influence / 100.0));
        })
        .width(FIELD)
        .build()
}

fn smoothing(clip: ClipId, key: KeyRef, smooth: Smooth) -> Element {
    let at = SMOOTHS
        .iter()
        .position(|candidate| *candidate == smooth)
        .unwrap_or(0);
    field(
        tr!("cut.easing.smooth"),
        segmented(at)
            .segments(SMOOTHS.map(wording::smooth))
            .control_size(ControlSize::Small)
            .focusless()
            .on_select(move |cut: &mut Cut, at: usize| cut.smooth_key(clip, &key, SMOOTHS[at]))
            .width(SMOOTHING),
    )
    .build()
}

fn actions(clip: &Clip, keys: &[KeyRef], selected_keys: &[KeyRef]) -> Element {
    let id = clip.id();
    let reachable = keys.iter().all(|key| clip.covers(key.at));
    let keyed = keys.iter().any(|key| clip.is_keyed(&key.property));
    let toggle = match selected_keys.is_empty() {
        true => {
            let adding = keys.to_vec();
            button(text(tr!("cut.key.add")))
                .enabled(reachable && !keys.is_empty())
                .on_press(move |cut: &mut Cut| cut.add_keys(id, &adding))
        }
        false => {
            let removing = selected_keys.to_vec();
            button(text(tr!("cut.key.remove")))
                .on_press(move |cut: &mut Cut| cut.remove_keys(id, &removing))
        }
    };
    let clearing = keys.to_vec();
    column! {
        toggle.small().ghost().focusless().fill_width();
        button(text(tr!("cut.key.clear")))
            .small()
            .ghost()
            .focusless()
            .enabled(keyed)
            .on_press(move |cut: &mut Cut| cut.clear_keys(id, &clearing))
            .fill_width();
    }
    .gap(Space::TIGHT)
    .fill_width()
    .build()
}
