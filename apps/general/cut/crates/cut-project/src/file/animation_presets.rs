use serde::{Deserialize, Serialize};

use super::is_span;
use super::tracks::{StoredEasing, restored_easing, stored_easing};
use crate::animation_presets::{
    AnimationPreset, AnimationPresetKind, AnimationPresets, Direction, Letters, LoopKind,
    LoopPreset, Order,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredAnimationPresets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    enter: Option<StoredAnimationPreset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exit: Option<StoredAnimationPreset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repeat: Option<StoredLoopPreset>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredAnimationPreset {
    transition: StoredKind,
    length: i64,
    easing: StoredEasing,
    strength: f32,
    direction: StoredDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    letters: Option<StoredLetters>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredLoopPreset {
    cycle: StoredLoopKind,
    period: i64,
    easing: StoredEasing,
    strength: f32,
    direction: StoredDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    letters: Option<StoredLetters>,
    #[serde(default, skip_serializing_if = "is_zero")]
    phase: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredLetters {
    delay: f32,
    order: StoredOrder,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum StoredOrder {
    Forward,
    Backward,
    Centre,
    Random { seed: u32 },
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredKind {
    Fade,
    Slide,
    Pop,
    Zoom,
    Spin,
    Wipe,
    Blur,
    Typewriter,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredLoopKind {
    Shake,
    Pulse,
    Float,
    Spin,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StoredDirection {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}

fn is_zero(value: &i64) -> bool {
    *value == 0
}

const DIRECTIONS: [(Direction, StoredDirection); 8] = [
    (Direction::Up, StoredDirection::Up),
    (Direction::Down, StoredDirection::Down),
    (Direction::Left, StoredDirection::Left),
    (Direction::Right, StoredDirection::Right),
    (Direction::UpLeft, StoredDirection::UpLeft),
    (Direction::UpRight, StoredDirection::UpRight),
    (Direction::DownLeft, StoredDirection::DownLeft),
    (Direction::DownRight, StoredDirection::DownRight),
];

const KINDS: [(AnimationPresetKind, StoredKind); 8] = [
    (AnimationPresetKind::Fade, StoredKind::Fade),
    (AnimationPresetKind::Slide, StoredKind::Slide),
    (AnimationPresetKind::Pop, StoredKind::Pop),
    (AnimationPresetKind::Zoom, StoredKind::Zoom),
    (AnimationPresetKind::Spin, StoredKind::Spin),
    (AnimationPresetKind::Wipe, StoredKind::Wipe),
    (AnimationPresetKind::Blur, StoredKind::Blur),
    (AnimationPresetKind::Typewriter, StoredKind::Typewriter),
];

const CYCLES: [(LoopKind, StoredLoopKind); 4] = [
    (LoopKind::Shake, StoredLoopKind::Shake),
    (LoopKind::Pulse, StoredLoopKind::Pulse),
    (LoopKind::Float, StoredLoopKind::Float),
    (LoopKind::Spin, StoredLoopKind::Spin),
];

pub(super) fn stored_direction(direction: Direction) -> StoredDirection {
    DIRECTIONS
        .iter()
        .find(|(candidate, _)| *candidate == direction)
        .map_or(StoredDirection::Left, |(_, stored)| *stored)
}

pub(super) fn restored_direction(direction: StoredDirection) -> Direction {
    DIRECTIONS
        .iter()
        .find(|(_, stored)| *stored == direction)
        .map_or(Direction::Left, |(candidate, _)| *candidate)
}

fn stored_letters(letters: Letters) -> StoredLetters {
    StoredLetters {
        delay: letters.delay,
        order: match letters.order {
            Order::Forward => StoredOrder::Forward,
            Order::Backward => StoredOrder::Backward,
            Order::Centre => StoredOrder::Centre,
            Order::Random(seed) => StoredOrder::Random { seed },
        },
    }
}

fn restored_letters(letters: &StoredLetters) -> Result<Letters, String> {
    if !letters.delay.is_finite() || letters.delay < 0.0 {
        return Err(format!("{} is not a delay between letters", letters.delay));
    }
    Ok(Letters {
        delay: letters.delay,
        order: match letters.order {
            StoredOrder::Forward => Order::Forward,
            StoredOrder::Backward => Order::Backward,
            StoredOrder::Centre => Order::Centre,
            StoredOrder::Random { seed } => Order::Random(seed),
        },
    })
}

fn stored_animation_preset(act: &AnimationPreset) -> StoredAnimationPreset {
    StoredAnimationPreset {
        transition: KINDS
            .iter()
            .find(|(candidate, _)| *candidate == act.kind)
            .map_or(StoredKind::Fade, |(_, stored)| *stored),
        length: act.length,
        easing: stored_easing(act.easing),
        strength: act.strength,
        direction: stored_direction(act.direction),
        letters: act.letters.map(stored_letters),
    }
}

fn restored_animation_preset(act: &StoredAnimationPreset) -> Result<AnimationPreset, String> {
    if !is_span(act.length) || !act.strength.is_finite() {
        return Err(format!(
            "an act lasts {} frames at strength {}",
            act.length, act.strength
        ));
    }
    Ok(AnimationPreset {
        kind: KINDS
            .iter()
            .find(|(_, stored)| *stored == act.transition)
            .map_or(AnimationPresetKind::Fade, |(candidate, _)| *candidate),
        length: act.length,
        easing: restored_easing(act.easing),
        strength: act.strength,
        direction: restored_direction(act.direction),
        letters: act.letters.as_ref().map(restored_letters).transpose()?,
    })
}

pub(super) fn stored_animation_presets(acts: &AnimationPresets) -> Option<StoredAnimationPresets> {
    (!acts.is_empty()).then(|| StoredAnimationPresets {
        enter: acts.in_preset.as_ref().map(stored_animation_preset),
        exit: acts.out_preset.as_ref().map(stored_animation_preset),
        repeat: acts.loop_preset.as_ref().map(|repeat| StoredLoopPreset {
            cycle: CYCLES
                .iter()
                .find(|(candidate, _)| *candidate == repeat.kind)
                .map_or(StoredLoopKind::Shake, |(_, stored)| *stored),
            period: repeat.period,
            easing: stored_easing(repeat.easing),
            strength: repeat.strength,
            direction: stored_direction(repeat.direction),
            letters: repeat.letters.map(stored_letters),
            phase: repeat.phase,
        }),
    })
}

pub(super) fn restored_animation_presets(
    acts: &StoredAnimationPresets,
    clip: u64,
) -> Result<AnimationPresets, String> {
    let wrapped = |reason: String| format!("clip {clip}: {reason}");
    let repeat = acts
        .repeat
        .as_ref()
        .map(|repeat| {
            if repeat.period < 1
                || !is_span(repeat.period)
                || !is_span(repeat.phase.saturating_abs())
                || !repeat.strength.is_finite()
            {
                return Err(format!("a repeat every {} frames", repeat.period));
            }
            Ok(LoopPreset {
                kind: CYCLES
                    .iter()
                    .find(|(_, stored)| *stored == repeat.cycle)
                    .map_or(LoopKind::Shake, |(candidate, _)| *candidate),
                period: repeat.period,
                easing: restored_easing(repeat.easing),
                strength: repeat.strength,
                direction: restored_direction(repeat.direction),
                letters: repeat.letters.as_ref().map(restored_letters).transpose()?,
                phase: repeat.phase,
            })
        })
        .transpose()
        .map_err(wrapped)?;
    Ok(AnimationPresets {
        in_preset: acts
            .enter
            .as_ref()
            .map(restored_animation_preset)
            .transpose()
            .map_err(wrapped)?,
        out_preset: acts
            .exit
            .as_ref()
            .map(restored_animation_preset)
            .transpose()
            .map_err(wrapped)?,
        loop_preset: repeat,
    })
}
