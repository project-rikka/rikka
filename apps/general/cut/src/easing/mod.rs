mod curves;
mod graph;
mod keyed;
mod segment;

use cut_project::{Clip, ClipId, Edge, EditPoint, KeyRef, Project};
use rikka_app::prelude::*;
use rikka_keyframe::{Easing, Family};
use rikka_timeline::family_label;
use rikka_widgets::{column, popover};

use crate::cut::{Cut, PresetSlot};
pub(crate) use curves::thumbnail;
pub(crate) use segment::{HandleChange, Segment};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Picking {
    pub(crate) clip: ClipId,
    pub(crate) target: Target,
    pub(crate) anchor: Anchor,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Target {
    Keys(Vec<KeyRef>),
    AnimationPreset(PresetSlot),
    Tween,
    Transition(Edge),
}

impl Picking {
    pub(crate) fn keys(clip: ClipId, keys: Vec<KeyRef>, anchor: Anchor) -> Picking {
        Picking {
            clip,
            target: Target::Keys(keys),
            anchor,
        }
    }

    pub(crate) fn animation_preset(clip: ClipId, slot: PresetSlot, anchor: Anchor) -> Picking {
        Picking {
            clip,
            target: Target::AnimationPreset(slot),
            anchor,
        }
    }

    pub(crate) fn tween(clip: ClipId, anchor: Anchor) -> Picking {
        Picking {
            clip,
            target: Target::Tween,
            anchor,
        }
    }

    pub(crate) fn transition(edit_point: EditPoint, anchor: Anchor) -> Picking {
        Picking {
            clip: edit_point.clip,
            target: Target::Transition(edit_point.edge),
            anchor,
        }
    }
}

pub(crate) fn held_keys(clip: &Clip, keys: &[KeyRef]) -> Vec<KeyRef> {
    keys.iter().filter(|key| clip.holds(key)).cloned().collect()
}

pub(crate) fn picker(project: &Project, picking: &Picking) -> Option<Element> {
    let (_, clip) = project.locate(picking.clip)?;
    let content = match &picking.target {
        Target::Keys(keys) => keyed::keyed(clip, keys, project.rate().per_second()),
        Target::AnimationPreset(slot) => preset_easing(clip, *slot)?,
        Target::Tween => tweened(clip)?,
        Target::Transition(edge) => passed(clip, *edge)?,
    };
    Some(
        popover(content)
            .on_dismiss(Cut::close_popup)
            .build()
            .keyed(("cut-easing", picking.clip, picking.target.clone()))
            .anchored(picking.anchor),
    )
}

fn preset_easing(clip: &Clip, slot: PresetSlot) -> Option<Element> {
    let easing = slot.easing(clip.animation_presets())?;
    let id = clip.id();
    Some(presets(easing, move |cut: &mut Cut, easing| {
        cut.ease_animation_preset(id, slot, easing);
    }))
}

fn tweened(clip: &Clip) -> Option<Element> {
    let easing = clip.tween()?.easing;
    let id = clip.id();
    Some(presets(easing, move |cut: &mut Cut, easing| {
        cut.ease_tween(id, easing);
    }))
}

fn passed(clip: &Clip, edge: Edge) -> Option<Element> {
    let easing = clip.transitions().at(edge)?.easing;
    Some(presets(easing, |cut: &mut Cut, easing| {
        cut.ease_transition(easing);
    }))
}

fn presets(easing: Easing, ease: impl Fn(&mut Cut, Easing) + Clone + 'static) -> Element {
    let straight = easing.family == Family::Linear;
    let eased = ease.clone();
    let linear = button(text(family_label(Family::Linear)))
        .small()
        .focusless()
        .on_press(move |cut: &mut Cut| eased(cut, Easing::LINEAR));
    let linear = match straight {
        true => linear.secondary(),
        false => linear.ghost(),
    };
    let grid = curves::grid(Some(easing), true, move |easing| {
        let ease = ease.clone();
        move |cut: &mut Cut| ease(cut, easing)
    });
    column! {
        text(tr!("cut.easing.curves")).label().semibold();
        linear;
        grid;
    }
    .gap(Space::SNUG)
    .build()
}
