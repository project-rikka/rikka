use serde::{Deserialize, Serialize};

use super::animation_presets::{StoredDirection, restored_direction, stored_direction};
use super::is_span;
use super::items::{restored_color, stored_color};
use super::tracks::{StoredEasing, restored_easing, stored_easing};
use crate::transitions::{
    Ends, Transition, TransitionKind as Kind, TransitionKnob as Knob, Tuning,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredEnds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    into: Option<StoredTransition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    out: Option<StoredTransition>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredTransition {
    kind: StoredKind,
    before: i64,
    after: i64,
    easing: StoredEasing,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    direction: Option<StoredDirection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    softness: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    blur: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    size: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shutter: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tint: Option<[f32; 4]>,
    sound: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredKind {
    CrossDissolve,
    FilmDissolve,
    AdditiveDissolve,
    Dip,
    BlurDissolve,
    NoiseDissolve,
    Wipe,
    Split,
    IrisRound,
    IrisBox,
    Push,
    SlideIn,
    SlideOut,
    CrossZoom,
    Turn,
    ZoomIn,
    ZoomOut,
    Spin,
    WhipPan,
    Stretch,
    Shake,
    Glitch,
    ChromaSplit,
    Flash,
    LightLeak,
    FilmBurn,
    Pixelate,
    LensWarp,
    LiquidWarp,
}

const KINDS: [(Kind, StoredKind); 29] = [
    (Kind::CrossDissolve, StoredKind::CrossDissolve),
    (Kind::FilmDissolve, StoredKind::FilmDissolve),
    (Kind::AdditiveDissolve, StoredKind::AdditiveDissolve),
    (Kind::Dip, StoredKind::Dip),
    (Kind::BlurDissolve, StoredKind::BlurDissolve),
    (Kind::NoiseDissolve, StoredKind::NoiseDissolve),
    (Kind::Wipe, StoredKind::Wipe),
    (Kind::Split, StoredKind::Split),
    (Kind::IrisRound, StoredKind::IrisRound),
    (Kind::IrisBox, StoredKind::IrisBox),
    (Kind::Push, StoredKind::Push),
    (Kind::SlideIn, StoredKind::SlideIn),
    (Kind::SlideOut, StoredKind::SlideOut),
    (Kind::CrossZoom, StoredKind::CrossZoom),
    (Kind::Turn, StoredKind::Turn),
    (Kind::ZoomIn, StoredKind::ZoomIn),
    (Kind::ZoomOut, StoredKind::ZoomOut),
    (Kind::Spin, StoredKind::Spin),
    (Kind::WhipPan, StoredKind::WhipPan),
    (Kind::Stretch, StoredKind::Stretch),
    (Kind::Shake, StoredKind::Shake),
    (Kind::Glitch, StoredKind::Glitch),
    (Kind::ChromaSplit, StoredKind::ChromaSplit),
    (Kind::Flash, StoredKind::Flash),
    (Kind::LightLeak, StoredKind::LightLeak),
    (Kind::FilmBurn, StoredKind::FilmBurn),
    (Kind::Pixelate, StoredKind::Pixelate),
    (Kind::LensWarp, StoredKind::LensWarp),
    (Kind::LiquidWarp, StoredKind::LiquidWarp),
];

pub(super) fn stored_ends(ends: &Ends) -> Option<StoredEnds> {
    (!ends.is_empty()).then(|| StoredEnds {
        into: ends.into.as_ref().map(stored_transition),
        out: ends.out.as_ref().map(stored_transition),
    })
}

pub(super) fn restored_ends(ends: &StoredEnds, clip: u64) -> Result<Ends, String> {
    let restored = |stored: &Option<StoredTransition>| {
        stored
            .as_ref()
            .map(restored_transition)
            .transpose()
            .map_err(|reason| format!("clip {clip}: {reason}"))
    };
    Ok(Ends {
        into: restored(&ends.into)?,
        out: restored(&ends.out)?,
    })
}

fn stored_transition(transition: &Transition) -> StoredTransition {
    let kind = transition.kind;
    let knob = |knob: Knob| {
        kind.has_knob(knob)
            .then(|| transition.tuning.value(knob))
            .flatten()
    };
    StoredTransition {
        kind: KINDS
            .iter()
            .find(|(candidate, _)| *candidate == kind)
            .map_or(StoredKind::CrossDissolve, |(_, stored)| *stored),
        before: transition.before,
        after: transition.after,
        easing: stored_easing(transition.easing),
        direction: (!kind.directions().is_empty()).then(|| stored_direction(transition.direction)),
        amount: knob(Knob::Amount),
        softness: knob(Knob::Softness),
        blur: knob(Knob::Blur),
        size: knob(Knob::Size),
        shutter: knob(Knob::Shutter),
        tint: kind
            .has_knob(Knob::Tint)
            .then(|| stored_color(transition.tuning.tint)),
        sound: transition.sound,
    }
}

fn restored_transition(stored: &StoredTransition) -> Result<Transition, String> {
    if !is_span(stored.before) || !is_span(stored.after) || stored.before + stored.after < 1 {
        return Err(format!(
            "a transition spans {} frames before and {} after its cut",
            stored.before, stored.after
        ));
    }
    let kind = KINDS
        .iter()
        .find(|(_, candidate)| *candidate == stored.kind)
        .map_or(Kind::CrossDissolve, |(kind, _)| *kind);
    let mut tuning = Tuning::default_for(kind);
    let knobs = [
        (Knob::Amount, stored.amount),
        (Knob::Softness, stored.softness),
        (Knob::Blur, stored.blur),
        (Knob::Size, stored.size),
        (Knob::Shutter, stored.shutter),
    ];
    for (knob, value) in knobs {
        if let Some(value) = value {
            if !value.is_finite() {
                return Err(format!("{value} is not a transition setting"));
            }
            tuning = tuning.with(kind, knob, value);
        }
    }
    if let Some(tint) = stored.tint {
        tuning = tuning.tinted(restored_color(tint)?);
    }
    Ok(Transition {
        kind,
        before: stored.before,
        after: stored.after,
        easing: restored_easing(stored.easing),
        direction: stored
            .direction
            .map_or(kind.direction(), restored_direction),
        tuning,
        sound: stored.sound,
    }
    .checked())
}
