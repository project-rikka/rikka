use std::mem::discriminant;
use std::ops::RangeInclusive;

use cut_project::{
    AnimationPresetKind, AnimationPresets, Clip, ClipId, Direction, Letters, LoopKind, Order,
};
use rikka_app::prelude::*;
use rikka_effects::{Scale, Unit};
use rikka_keyframe::Easing;

use super::rows::{Apply, easing_row, group, picker_row, slider_row, toggle_row};
use crate::controls::toggle;
use crate::cut::{Cut, PresetSlot, PresetStyle};
use crate::wording;

const EASING: &str = "cut-act-easing";
const LONGEST_PERIOD: f32 = 300.0;
const LONGEST_DELAY: f32 = 15.0;
const GLYPH: f32 = 16.0;
const COMPASS: f32 = 96.0;
const ORDERS: [Order; 4] = [
    Order::Forward,
    Order::Backward,
    Order::Centre,
    Order::Random(1),
];
pub(super) const POINTS: [Option<Direction>; 9] = [
    Some(Direction::UpLeft),
    Some(Direction::Up),
    Some(Direction::UpRight),
    Some(Direction::Left),
    None,
    Some(Direction::Right),
    Some(Direction::DownLeft),
    Some(Direction::Down),
    Some(Direction::DownRight),
];

struct Moves {
    span: i64,
    easing: Easing,
    strength: f32,
    direction: Direction,
    letters: Option<Letters>,
}

struct Strength {
    range: RangeInclusive<f32>,
    scale: f32,
    unit: Unit,
}

impl Strength {
    fn for_style(style: PresetStyle) -> Option<Strength> {
        let percent = |most: f32| Strength {
            range: 0.0..=most,
            scale: 100.0,
            unit: Unit::Percent,
        };
        let pixels = |most: f32| Strength {
            range: 0.0..=most,
            scale: 1.0,
            unit: Unit::Pixels,
        };
        let degrees = |most: f32| Strength {
            range: -most..=most,
            scale: 1.0,
            unit: Unit::Degrees,
        };
        match style {
            PresetStyle::InOut(AnimationPresetKind::Fade | AnimationPresetKind::Pop) => {
                Some(percent(100.0))
            }
            PresetStyle::InOut(AnimationPresetKind::Zoom) => Some(percent(500.0)),
            PresetStyle::InOut(AnimationPresetKind::Slide) => Some(pixels(2_000.0)),
            PresetStyle::InOut(AnimationPresetKind::Spin) => Some(degrees(720.0)),
            PresetStyle::InOut(AnimationPresetKind::Blur) => Some(pixels(200.0)),
            PresetStyle::InOut(AnimationPresetKind::Wipe | AnimationPresetKind::Typewriter) => None,
            PresetStyle::Loop(LoopKind::Shake) => Some(pixels(200.0)),
            PresetStyle::Loop(LoopKind::Pulse) => Some(percent(100.0)),
            PresetStyle::Loop(LoopKind::Float) => Some(pixels(500.0)),
            PresetStyle::Loop(LoopKind::Spin) => Some(degrees(1_440.0)),
        }
    }
}

fn directed(style: PresetStyle) -> bool {
    matches!(
        style,
        PresetStyle::InOut(AnimationPresetKind::Slide | AnimationPresetKind::Wipe)
            | PresetStyle::Loop(LoopKind::Shake | LoopKind::Float)
    )
}

fn name(style: PresetStyle) -> Label {
    match style {
        PresetStyle::InOut(kind) => wording::animation_preset_kind(kind),
        PresetStyle::Loop(cycle) => wording::cycle(cycle),
    }
}

fn tuned<T: Copy + 'static>(
    set: impl Fn(&mut AnimationPresets, T) + Clone + 'static,
) -> (impl Apply<T>, impl Apply<T>) {
    let previewed = set.clone();
    (
        move |cut: &mut Cut, value: T| {
            cut.preview_animation_presets(|animation_presets| previewed(animation_presets, value))
        },
        move |cut: &mut Cut, value: T| {
            cut.change_animation_presets(|animation_presets| set(animation_presets, value))
        },
    )
}

pub(crate) fn animation_presets(clip: &Clip) -> impl View {
    let lettered = clip.graphic().is_some_and(|graphic| graphic.is_text());
    let rows = PresetSlot::ALL
        .into_iter()
        .flat_map(|slot| rows(clip, slot, lettered))
        .collect();
    group(tr!("cut.acts.group"), rows)
}

fn rows(clip: &Clip, slot: PresetSlot, lettered: bool) -> Vec<Element> {
    let animation_presets = clip.animation_presets();
    let style = slot.style(animation_presets);
    let mut rows = vec![kinds(slot, &slot.styles(lettered), style)];
    let moves = slot.read(animation_presets, |part| Moves {
        span: *part.span(),
        easing: *part.easing(),
        strength: *part.strength(),
        direction: *part.direction(),
        letters: *part.letters(),
    });
    let (Some(style), Some(animation_preset_moves)) = (style, moves) else {
        return rows;
    };
    rows.push(span(slot, animation_preset_moves.span, clip.length()));
    rows.push(easing(clip.id(), slot, animation_preset_moves.easing));
    if let Some(scale) = Strength::for_style(style) {
        rows.push(strength(slot, animation_preset_moves.strength, scale));
    }
    if directed(style) {
        rows.push(compass(slot, animation_preset_moves.direction));
    }
    if lettered {
        rows.extend(letters(slot, animation_preset_moves.letters));
    }
    rows
}

fn kinds(slot: PresetSlot, styles: &[PresetStyle], style: Option<PresetStyle>) -> Element {
    let choices: Vec<Option<PresetStyle>> = std::iter::once(None)
        .chain(styles.iter().copied().map(Some))
        .collect();
    let at = choices.iter().position(|choice| *choice == style);
    let options = choices
        .iter()
        .map(|choice| choice.map_or_else(|| tr!("cut.acts.none"), name))
        .collect();
    picker_row(
        wording::preset_slot(slot),
        at,
        options,
        move |cut: &mut Cut, at: usize| {
            let selected = choices.get(at).copied().flatten();
            cut.change_animation_presets(|animation_presets| {
                slot.begin(animation_presets, selected)
            });
        },
    )
}

fn span(slot: PresetSlot, span: i64, length: i64) -> Element {
    let (label, most) = match slot {
        PresetSlot::Loop => (tr!("cut.acts.period"), LONGEST_PERIOD),
        PresetSlot::In | PresetSlot::Out => (tr!("cut.acts.length"), length.max(2) as f32),
    };
    slider_row(
        label,
        span as f32,
        Scale::new(1.0..=most).unit(Unit::Frames),
        tuned(
            move |animation_presets: &mut AnimationPresets, frames: f32| {
                slot.tune(animation_presets, |part| {
                    *part.span() = frames.round().max(1.0) as i64
                });
            },
        ),
        None,
    )
}

fn easing(clip: ClipId, slot: PresetSlot, easing: Easing) -> Element {
    easing_row(
        tr!("cut.acts.easing"),
        (EASING, clip, slot),
        easing,
        move |cut: &mut Cut, anchor: Anchor| cut.open_animation_preset_easing(slot, anchor),
    )
}

fn strength(slot: PresetSlot, strength: f32, scale: Strength) -> Element {
    let factor = scale.scale;
    slider_row(
        tr!("cut.acts.strength"),
        strength * factor,
        Scale::new(scale.range).unit(scale.unit),
        tuned(
            move |animation_presets: &mut AnimationPresets, value: f32| {
                slot.tune(animation_presets, |part| *part.strength() = value / factor);
            },
        ),
        None,
    )
}

pub(super) fn arrow(direction: Direction) -> MaterialSymbol {
    match direction {
        Direction::Up => MaterialSymbol::NORTH,
        Direction::Down => MaterialSymbol::SOUTH,
        Direction::Left => MaterialSymbol::WEST,
        Direction::Right => MaterialSymbol::EAST,
        Direction::UpLeft => MaterialSymbol::NORTH_WEST,
        Direction::UpRight => MaterialSymbol::NORTH_EAST,
        Direction::DownLeft => MaterialSymbol::SOUTH_WEST,
        Direction::DownRight => MaterialSymbol::SOUTH_EAST,
    }
}

fn compass(slot: PresetSlot, selected: Direction) -> Element {
    let cells = POINTS.map(|point| match point {
        Some(direction) => toggle(
            arrow(direction),
            GLYPH,
            direction == selected,
            move |cut: &mut Cut| {
                cut.change_animation_presets(|animation_presets| {
                    slot.tune(animation_presets, |part| *part.direction() = direction)
                });
            },
        )
        .build(),
        None => spacer().build(),
    });
    field(tr!("cut.acts.direction"), grid_of(3, cells).width(COMPASS)).build()
}

fn letters(slot: PresetSlot, letters: Option<Letters>) -> Vec<Element> {
    let mut rows = vec![toggle_row(
        tr!("cut.acts.letters"),
        letters.is_some(),
        move |cut: &mut Cut, on: bool| {
            cut.change_animation_presets(|animation_presets| {
                slot.tune(animation_presets, |part| {
                    *part.letters() = on.then(|| Letters::new(Order::Forward));
                });
            });
        },
    )];
    let Some(letters) = letters else {
        return rows;
    };
    rows.push(slider_row(
        tr!("cut.acts.delay"),
        letters.delay,
        Scale::new(0.0..=LONGEST_DELAY)
            .unit(Unit::Frames)
            .decimals(1),
        tuned(
            move |animation_presets: &mut AnimationPresets, delay: f32| {
                slot.tune(animation_presets, |part| {
                    if let Some(letters) = part.letters() {
                        letters.delay = delay;
                    }
                });
            },
        ),
        None,
    ));
    let at = ORDERS
        .iter()
        .position(|order| discriminant(order) == discriminant(&letters.order));
    rows.push(picker_row(
        tr!("cut.acts.order"),
        at,
        ORDERS.map(wording::order).to_vec(),
        move |cut: &mut Cut, at: usize| {
            cut.change_animation_presets(|animation_presets| {
                slot.tune(animation_presets, |part| {
                    if let Some(letters) = part.letters()
                        && discriminant(&letters.order) != discriminant(&ORDERS[at])
                    {
                        letters.order = ORDERS[at];
                    }
                });
            });
        },
    ));
    rows
}
