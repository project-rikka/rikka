use std::collections::BTreeSet;
use std::ops::Range;

use crate::animation_presets::mixed;
use crate::clip::{Clip, ClipId};
use crate::project::{Edge, Project};

use super::edit_point::EditPoint;
use super::transition::Transition;

#[derive(Clone, Debug, PartialEq)]
pub struct TransitionSpan<'a> {
    pub edit_point: EditPoint,
    pub layer: usize,
    pub transition: &'a Transition,
    pub out: Option<&'a Clip>,
    pub into: Option<&'a Clip>,
    pub cut: i64,
    pub span: Range<i64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrozenFrames {
    pub out: i64,
    pub into: i64,
}

impl TransitionSpan<'_> {
    #[must_use]
    pub fn covers(&self, frame: i64) -> bool {
        self.span.contains(&frame)
    }

    #[must_use]
    pub fn length(&self) -> i64 {
        self.span.end - self.span.start
    }

    #[must_use]
    pub fn progress(&self, at: f64) -> f64 {
        let length = self.length().max(1) as f64;
        ((at - self.span.start as f64) / length).clamp(0.0, 1.0)
    }
    #[must_use]
    pub fn seed(&self) -> u32 {
        let id = |clip: Option<&Clip>| clip.map_or(0, |clip| clip.id().0);
        (mixed(id(self.out) << 32 ^ id(self.into)) >> 32) as u32
    }
}

#[derive(Clone, Copy, Debug)]
struct Parts {
    head: i64,
    tail: i64,
}

impl Project {
    #[must_use]
    pub fn transition_spans(&self, layer: usize) -> Vec<TransitionSpan<'_>> {
        let Some(layer_ref) = self.layer(layer) else {
            return Vec::new();
        };
        let clips = layer_ref.clips();
        let joined: Vec<bool> = (0..clips.len())
            .map(|at| {
                clips
                    .get(at + 1)
                    .is_some_and(|next| self.joins(&clips[at], next))
            })
            .collect();
        let follows = |at: usize| at > 0 && joined[at - 1];
        let mut parts = vec![Parts { head: 0, tail: 0 }; clips.len()];
        for (at, clip) in clips.iter().enumerate() {
            if let Some(into) = &clip.transitions.into {
                match follows(at) {
                    true => {
                        parts[at].head = into.after;
                        parts[at - 1].tail = into.before;
                    }
                    false => parts[at].head = into.length(),
                }
            }
            if let Some(out) = clip.transitions.out.filter(|_| !joined[at]) {
                parts[at].tail = out.length();
            }
        }
        for (part, clip) in parts.iter_mut().zip(clips) {
            let total = part.head + part.tail;
            if total > clip.length() {
                part.head = part.head * clip.length() / total;
                part.tail = clip.length() - part.head;
            }
        }
        let mut transition_spans = Vec::new();
        for (at, clip) in clips.iter().enumerate() {
            if let Some(into) = &clip.transitions.into {
                let (out, from) = match follows(at) {
                    true => (Some(&clips[at - 1]), clip.start() - parts[at - 1].tail),
                    false => (None, clip.start()),
                };
                transition_spans.push(TransitionSpan {
                    edit_point: EditPoint::head(clip.id()),
                    layer,
                    transition: into,
                    out,
                    into: Some(clip),
                    cut: clip.start(),
                    span: from..clip.start() + parts[at].head,
                });
            }
            if let Some(out) = clip.transitions.out.as_ref().filter(|_| !joined[at]) {
                transition_spans.push(TransitionSpan {
                    edit_point: EditPoint::tail(clip.id()),
                    layer,
                    transition: out,
                    out: Some(clip),
                    into: None,
                    cut: clip.end(),
                    span: clip.end() - parts[at].tail..clip.end(),
                });
            }
        }
        transition_spans.retain(|transition_span| transition_span.length() > 0);
        transition_spans
    }

    #[must_use]
    pub fn transition_span(&self, edit_point: EditPoint) -> Option<TransitionSpan<'_>> {
        let edit_point = self.canonical_edit_point(edit_point)?;
        let (layer, _) = self.locate(edit_point.clip)?;
        self.transition_spans(layer)
            .into_iter()
            .find(|transition_span| transition_span.edit_point == edit_point)
    }

    #[must_use]
    pub fn transition_span_at(&self, layer: usize, frame: i64) -> Option<TransitionSpan<'_>> {
        self.transition_spans(layer)
            .into_iter()
            .find(|transition_span| transition_span.covers(frame))
    }

    #[must_use]
    pub fn transition(&self, edit_point: EditPoint) -> Option<&Transition> {
        let edit_point = self.canonical_edit_point(edit_point)?;
        let (_, clip) = self.locate(edit_point.clip)?;
        clip.transitions.at(edit_point.edge)
    }

    #[must_use]
    pub fn joins(&self, before: &Clip, after: &Clip) -> bool {
        before.end() == after.start() && self.is_visual(before) && self.is_visual(after)
    }

    #[must_use]
    pub fn canonical_edit_point(&self, edit_point: EditPoint) -> Option<EditPoint> {
        let (layer, clip) = self.locate(edit_point.clip)?;
        if edit_point.edge == Edge::Head {
            return Some(edit_point);
        }
        let next = self.layers[layer]
            .clips()
            .iter()
            .find(|next| self.joins(clip, next));
        Some(next.map_or(edit_point, |next| EditPoint::head(next.id())))
    }

    #[must_use]
    pub fn edit_points_among(&self, clips: &BTreeSet<ClipId>) -> Vec<EditPoint> {
        self.layers()
            .iter()
            .flat_map(|layer| layer.clips().windows(2))
            .filter_map(|pair| match pair {
                [before, after]
                    if clips.contains(&before.id())
                        && clips.contains(&after.id())
                        && self.joins(before, after) =>
                {
                    Some(EditPoint::head(after.id()))
                }
                _ => None,
            })
            .collect()
    }

    #[must_use]
    pub fn frozen_frames(&self, transition_span: &TransitionSpan<'_>) -> FrozenFrames {
        let out = transition_span
            .out
            .and_then(|clip| Some((clip, clip.media()?)));
        let out = out.map_or(0, |(clip, media)| {
            let frozen = clip.frame_reaching(self.media_frames(media));
            transition_span.span.end
                - frozen
                    .max(transition_span.cut)
                    .max(transition_span.span.start)
        });
        let into = transition_span.into.filter(|clip| clip.media().is_some());
        let into = into.map_or(0, |clip| {
            clip.frame_reaching(0)
                .min(transition_span.cut)
                .min(transition_span.span.end)
                - transition_span.span.start
        });
        FrozenFrames {
            out: out.max(0),
            into: into.max(0),
        }
    }
}
