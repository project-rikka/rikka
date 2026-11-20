use std::cmp::Ordering;

use rikka_text_font::{Synthesis, Tag};

use crate::face::{AxisRange, Face, FaceId, Slope};
use crate::matching::face_match::{ChainId, FaceMatch, StyleRequest, Variations};
use crate::style::FontStyle;

const WEIGHT_AXIS: Tag = Tag::new(b"wght");
const WIDTH_AXIS: Tag = Tag::new(b"wdth");
const ITALIC_AXIS: Tag = Tag::new(b"ital");
const SLANT_AXIS: Tag = Tag::new(b"slnt");
const OPTICAL_SIZE_AXIS: Tag = Tag::new(b"opsz");
const ITALIC_OBLIQUE_ANGLE: f32 = 14.0;
const BOLD_THRESHOLD: f32 = 600.0;
const NORMAL_WIDTH: f32 = 100.0;
const LOW_WEIGHT_PIVOT: f32 = 400.0;
const HIGH_WEIGHT_PIVOT: f32 = 500.0;

type Key = (u8, f32);

#[derive(Clone, Copy, Debug, PartialEq)]
enum SlopeMatch {
    Upright,
    Italic,
    Oblique(f32),
    Synthesized,
}

pub(crate) fn best_face(
    faces: &[Face],
    candidates: impl IntoIterator<Item = FaceId>,
    request: &StyleRequest,
    chain: ChainId,
) -> Option<FaceMatch> {
    let mut best: Option<([Key; 3], FaceId, SlopeMatch)> = None;
    for id in candidates {
        let record = &faces[id.index()].record;
        let (style, fit) = style_key(record.slope, request.style);
        let score = [
            width_key(&record.width, request.width.0),
            style,
            weight_key(&record.weight, request.weight.0),
        ];
        if best
            .as_ref()
            .is_none_or(|(current, _, _)| compare(&score, current) == Ordering::Less)
        {
            best = Some((score, id, fit));
        }
    }
    let (_, id, fit) = best?;
    Some(matched_face(&faces[id.index()], id, fit, request, chain))
}

fn compare(left: &[Key; 3], right: &[Key; 3]) -> Ordering {
    left.iter()
        .zip(right)
        .map(|(a, b)| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)))
        .find(|ordering| ordering.is_ne())
        .unwrap_or(Ordering::Equal)
}

fn width_key(range: &AxisRange, desired: f32) -> Key {
    if range.contains(desired) {
        return (0, 0.0);
    }
    let wider = range.min > desired;
    let distance = if wider {
        range.min - desired
    } else {
        desired - range.max
    };
    let preferred = if desired <= NORMAL_WIDTH {
        !wider
    } else {
        wider
    };
    (if preferred { 1 } else { 2 }, distance)
}

fn weight_key(range: &AxisRange, desired: f32) -> Key {
    if range.contains(desired) {
        return (0, 0.0);
    }
    let heavier = range.min > desired;
    let distance = if heavier {
        range.min - desired
    } else {
        desired - range.max
    };
    let tier = if (LOW_WEIGHT_PIVOT..=HIGH_WEIGHT_PIVOT).contains(&desired) {
        if !heavier {
            2
        } else if range.min <= HIGH_WEIGHT_PIVOT {
            1
        } else {
            3
        }
    } else if desired < LOW_WEIGHT_PIVOT {
        if heavier { 2 } else { 1 }
    } else if heavier {
        1
    } else {
        2
    };
    (tier, distance)
}

fn style_key(slope: Slope, style: FontStyle) -> (Key, SlopeMatch) {
    match style {
        FontStyle::Oblique(angle) if angle > 0.0 => oblique_key(slope, angle),
        FontStyle::Italic => {
            if slope.italic {
                ((0, 0.0), SlopeMatch::Italic)
            } else if let Some((low, high)) = slope.oblique {
                let angle = ITALIC_OBLIQUE_ANGLE.clamp(low, high);
                (
                    (1, (angle - ITALIC_OBLIQUE_ANGLE).abs()),
                    SlopeMatch::Oblique(angle),
                )
            } else {
                ((2, 0.0), SlopeMatch::Synthesized)
            }
        }
        FontStyle::Normal | FontStyle::Oblique(_) => {
            if slope.upright {
                ((0, 0.0), SlopeMatch::Upright)
            } else if let Some((low, _)) = slope.oblique {
                ((1, low), SlopeMatch::Oblique(low))
            } else {
                ((2, 0.0), SlopeMatch::Italic)
            }
        }
    }
}

fn oblique_key(slope: Slope, angle: f32) -> (Key, SlopeMatch) {
    if let Some((low, high)) = slope.oblique {
        if low <= angle && angle <= high {
            return ((0, 0.0), SlopeMatch::Oblique(angle));
        }
        if low > angle {
            return ((1, low - angle), SlopeMatch::Oblique(low));
        }
        return ((2, angle - high), SlopeMatch::Oblique(high));
    }
    if slope.italic {
        ((3, 0.0), SlopeMatch::Italic)
    } else {
        ((4, 0.0), SlopeMatch::Synthesized)
    }
}

fn matched_face(
    face: &Face,
    id: FaceId,
    fit: SlopeMatch,
    request: &StyleRequest,
    chain: ChainId,
) -> FaceMatch {
    let mut variations = Variations::default();
    for axis in face.axes() {
        let clamp = |value: f32| value.clamp(axis.min, axis.max);
        match axis.tag {
            WEIGHT_AXIS => variations.set(axis.tag, clamp(request.weight.0)),
            WIDTH_AXIS => variations.set(axis.tag, clamp(request.width.0)),
            ITALIC_AXIS => {
                let value = if fit == SlopeMatch::Italic { 1.0 } else { 0.0 };
                variations.set(axis.tag, clamp(value));
            }
            SLANT_AXIS => match fit {
                SlopeMatch::Oblique(angle) => variations.set(axis.tag, clamp(-angle)),
                SlopeMatch::Upright | SlopeMatch::Synthesized => {
                    variations.set(axis.tag, clamp(0.0))
                }
                SlopeMatch::Italic => {}
            },
            OPTICAL_SIZE_AXIS => variations.enable_optical_size(),
            _ => {}
        }
    }
    FaceMatch {
        face: id,
        synthesis: Synthesis {
            bold: request.weight.0 >= BOLD_THRESHOLD && face.record.weight.max < BOLD_THRESHOLD,
            oblique: fit == SlopeMatch::Synthesized,
        },
        variations,
        request: *request,
        chain,
    }
}
