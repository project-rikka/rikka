use cut_project::Interpolation;
use rikka_app::prelude::*;
use rikka_effects::Scale;
use rikka_effects_forms::ScaleControls;
use rikka_keyframe::{Easing, Family};
use rikka_timeline::family_label;
use rikka_widgets::{column, row};
use std::hash::Hash;

use crate::cut::{Adjustable, Cut};
use crate::easing::thumbnail;
use crate::wording;

pub(crate) const FIELD: f32 = 112.0;
const CHOICE: f32 = 160.0;

pub(crate) trait Set<S, T>: Fn(&mut S, T) + Clone + 'static {}

impl<S, T, F: Fn(&mut S, T) + Clone + 'static> Set<S, T> for F {}

pub(crate) trait Apply<T>: Fn(&mut Cut, T) + Clone + 'static {}

impl<T, F: Fn(&mut Cut, T) + Clone + 'static> Apply<T> for F {}

fn styled<S: Adjustable, T: 'static>(set: impl Set<S, T>) -> impl Apply<T> {
    move |cut: &mut Cut, value: T| cut.restyle(|styled: &mut S| set(styled, value))
}

fn previewed<S: Adjustable, T: 'static>(set: impl Set<S, T>) -> impl Apply<T> {
    move |cut: &mut Cut, value: T| cut.preview_style(|styled: &mut S| set(styled, value))
}

pub(crate) fn group(title: Label, rows: Vec<Element>) -> impl View {
    group_with(title, None, rows)
}

pub(crate) fn group_with(title: Label, trailing: Option<Element>, rows: Vec<Element>) -> impl View {
    let mut children = vec![
        row! {
            text(title).label().semibold().grow(1.0);
            trailing;
        }
        .align_items(Align::Center)
        .fill_width()
        .build(),
    ];
    children.extend(rows);
    column_of(children).gap(Space::SNUG).fill_width()
}

fn number_input(scale: &Scale, value: f32, commit: impl Apply<f32>) -> NumberField {
    scale
        .field(value)
        .control_size(ControlSize::Small)
        .on_commit(commit)
}

pub(crate) fn number<S: Adjustable>(
    label: Label,
    value: f32,
    scale: Scale,
    set: impl Set<S, f32>,
) -> Element {
    number_with(label, value, scale, set, None)
}

pub(crate) fn number_with<S: Adjustable>(
    label: Label,
    value: f32,
    scale: Scale,
    set: impl Set<S, f32>,
    trailing: Option<Element>,
) -> Element {
    number_row(label, value, scale, styled(set), trailing)
}

pub(crate) fn number_row(
    label: Label,
    value: f32,
    scale: Scale,
    commit: impl Apply<f32>,
    trailing: Option<Element>,
) -> Element {
    field(
        label,
        row! {
            number_input(&scale, value, commit).width(FIELD);
            trailing;
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
    .build()
}

pub(crate) fn pair_with<S: Adjustable>(
    label: Label,
    point: Point,
    scale: Scale,
    sets: (impl Set<S, f32>, impl Set<S, f32>),
    trailing: Option<Element>,
) -> Element {
    column! {
        text(label).color(TextColor::Muted).no_wrap();
        row! {
            number_input(&scale, point.x, styled(sets.0)).width(FIELD);
            number_input(&scale, point.y, styled(sets.1)).width(FIELD);
            spacer().grow(1.0);
            trailing;
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::TIGHT)
    .fill_width()
    .build()
}

pub(crate) fn sliding<S: Adjustable>(
    label: Label,
    value: f32,
    scale: Scale,
    set: impl Set<S, f32>,
) -> Element {
    sliding_with(label, value, scale, set, None)
}

pub(crate) fn sliding_with<S: Adjustable>(
    label: Label,
    value: f32,
    scale: Scale,
    set: impl Set<S, f32>,
    trailing: Option<Element>,
) -> Element {
    let preview = previewed(set.clone());
    slider_row(label, value, scale, (preview, styled(set)), trailing)
}

pub(crate) fn slider_row(
    label: Label,
    value: f32,
    scale: Scale,
    (preview, commit): (impl Apply<f32>, impl Apply<f32>),
    trailing: Option<Element>,
) -> Element {
    let released = commit.clone();
    let slid = scale
        .slider(value)
        .control_size(ControlSize::Small)
        .on_change(preview)
        .on_commit(released)
        .grow(1.0);
    column! {
        text(label).color(TextColor::Muted).no_wrap();
        row! {
            slid;
            number_input(&scale, value, commit).width(FIELD);
            trailing;
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::TIGHT)
    .fill_width()
    .build()
}

pub(crate) fn colour<S: Adjustable>(
    label: Label,
    color: Color,
    set: impl Set<S, Color>,
) -> Element {
    colour_with(label, color, set, None)
}

pub(crate) fn colour_with<S: Adjustable>(
    label: Label,
    color: Color,
    set: impl Set<S, Color>,
    trailing: Option<Element>,
) -> Element {
    field(
        label,
        row! {
            color_well(color)
                .control_size(ControlSize::Small)
                .on_change(previewed(set.clone()))
                .on_commit(styled(set));
            trailing;
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
    .build()
}

pub(crate) fn switch_row<S: Adjustable>(label: Label, on: bool, set: impl Set<S, bool>) -> Element {
    toggle_row(label, on, styled(set))
}

pub(crate) fn toggle_row(label: Label, on: bool, commit: impl Apply<bool>) -> Element {
    checkbox(on)
        .label(text(label).no_wrap())
        .control_size(ControlSize::Small)
        .on_toggle(commit)
        .build()
}

pub(crate) fn choice<S: Adjustable, T: Copy + PartialEq + 'static, const N: usize>(
    label: Label,
    all: [T; N],
    selected: T,
    name: fn(T) -> Label,
    set: impl Set<S, T>,
) -> Element {
    let at = all.iter().position(|one| *one == selected);
    let commit = styled(set);
    picker_row(
        label,
        at,
        all.map(name).to_vec(),
        move |cut: &mut Cut, at: usize| {
            commit(cut, all[at]);
        },
    )
}

pub(crate) fn picker_row(
    label: Label,
    at: Option<usize>,
    options: Vec<Label>,
    commit: impl Apply<usize>,
) -> Element {
    field(
        label,
        combo(at)
            .options(options)
            .control_size(ControlSize::Small)
            .focusless()
            .on_select(commit)
            .width(CHOICE),
    )
    .build()
}

pub(crate) fn easing_row(
    label: Label,
    key: impl Hash + Clone,
    easing: Easing,
    open: impl Apply<Anchor>,
) -> Element {
    let anchor = Anchor::new(key.clone(), AnchorEdge::BelowEnd);
    let named = match easing.family {
        Family::Linear => family_label(Family::Linear),
        _ => wording::interpolation(Interpolation::Preset(easing)),
    };
    field(
        label,
        row! {
            thumbnail(easing, move |cut: &mut Cut| open(cut, anchor))
                .build()
                .keyed(key);
            text(named).no_wrap();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
    .build()
}
