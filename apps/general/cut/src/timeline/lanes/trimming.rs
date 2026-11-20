use std::ops::RangeInclusive;

use cut_project::{ClipId, Edge, Edit, EditPoint, Project, Snap};
use rikka_app::prelude::*;

use super::grab::{frames, reach};
use crate::timeline::scale::Scale;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Trim {
    Plain { clip: ClipId, edge: Edge },
    Ripple { clip: ClipId, edge: Edge },
    Roll { edit_point: EditPoint },
    Slip { clip: ClipId },
    Slide { clip: ClipId },
}

impl Trim {
    pub(crate) fn moving(self, project: &Project) -> Vec<ClipId> {
        match self {
            Trim::Roll { edit_point } => project
                .joint(edit_point)
                .map(|joint| joint.out.into_iter().chain(joint.into).collect())
                .unwrap_or_default(),
            Trim::Plain { clip, .. }
            | Trim::Ripple { clip, .. }
            | Trim::Slip { clip }
            | Trim::Slide { clip } => vec![clip],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Trimming {
    at: Point,
    pub(crate) trim: Trim,
    limits: RangeInclusive<i64>,
    snap: Option<Snap>,
    edges: Vec<i64>,
    pace: f64,
    point: i64,
    still: bool,
    peek: Peek,
    pub(crate) by: i64,
    pub(crate) snapped: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Peek {
    First(i64),
    Last(i64),
    Fixed(i64),
}

impl Trimming {
    pub(crate) fn begin(
        at: Point,
        trim: Trim,
        project: &Project,
        snap: Option<Snap>,
        playhead: i64,
    ) -> Option<Trimming> {
        let clip_of = |id: ClipId| project.locate(id).map(|(_, clip)| clip);
        let (limits, edges, pace, point, still, peek) = match trim {
            Trim::Plain { clip, edge } | Trim::Ripple { clip, edge } => {
                let clip_ref = clip_of(clip)?;
                let limits = match trim {
                    Trim::Ripple { .. } => project.ripple_limits(clip, edge),
                    _ => project.trim_limits(&[clip], edge),
                };
                let (point, peek) = match (edge, trim) {
                    (Edge::Head, Trim::Ripple { .. }) => {
                        (clip_ref.start(), Peek::Fixed(clip_ref.start()))
                    }
                    (Edge::Head, _) => (clip_ref.start(), Peek::First(clip_ref.start())),
                    (Edge::Tail, _) => (clip_ref.end(), Peek::Last(clip_ref.end())),
                };
                let still = matches!((edge, trim), (Edge::Head, Trim::Ripple { .. }));
                (limits, vec![point], 1.0, point, still, peek)
            }
            Trim::Roll { edit_point } => {
                let joint = project.joint(edit_point)?;
                let peek = match joint.into {
                    Some(_) => Peek::First(joint.at),
                    None => Peek::Last(joint.at),
                };
                let limits = project.roll_limits(edit_point);
                (limits, vec![joint.at], 1.0, joint.at, false, peek)
            }
            Trim::Slip { clip } => {
                let clip_ref = clip_of(clip)?;
                let frame = match clip_ref.covers(playhead) {
                    true => playhead,
                    false => clip_ref.start(),
                };
                let middle = (clip_ref.start() + clip_ref.end()) / 2;
                let limits = project.slip_limits(&[clip]);
                (
                    limits,
                    Vec::new(),
                    -clip_ref.speed(),
                    middle,
                    true,
                    Peek::Fixed(frame),
                )
            }
            Trim::Slide { clip } => {
                let clip_ref = clip_of(clip)?;
                let abutted = project
                    .joint(EditPoint::head(clip))
                    .is_some_and(|joint| joint.out.is_some());
                let peek = match abutted {
                    true => Peek::Last(clip_ref.start()),
                    false => Peek::First(clip_ref.end()),
                };
                let limits = project.slide_limits(clip);
                (
                    limits,
                    vec![clip_ref.start(), clip_ref.end()],
                    1.0,
                    clip_ref.start(),
                    false,
                    peek,
                )
            }
        };
        Some(Trimming {
            at,
            trim,
            limits,
            snap,
            edges,
            pace,
            point,
            still,
            peek,
            by: 0,
            snapped: None,
        })
    }

    pub(crate) fn drag(&mut self, to: Point, scale: &Scale) {
        let clamp = |by: i64| by.clamp(*self.limits.start(), *self.limits.end());
        let moved = frames(scale, to.x - self.at.x) as f64 * self.pace;
        let mut by = clamp(moved.round() as i64);
        self.snapped = None;
        if let Some(snap) = &self.snap {
            let edges: Vec<i64> = self.edges.iter().map(|edge| edge + by).collect();
            if let Some(pull) = snap.pull(&edges, reach(scale))
                && clamp(by + pull) == by + pull
            {
                by += pull;
                self.snapped = edges
                    .iter()
                    .map(|edge| edge + pull)
                    .find(|point| snap.pull(&[*point], 0) == Some(0));
            }
        }
        self.by = by;
    }

    pub(crate) fn has_moved(&self) -> bool {
        self.by != 0
    }

    pub(crate) fn point(&self) -> i64 {
        match self.still {
            true => self.point,
            false => self.point + self.by,
        }
    }

    pub(crate) fn peek(&self) -> i64 {
        match self.peek {
            Peek::First(frame) => frame + self.by,
            Peek::Last(frame) => (frame + self.by - 1).max(0),
            Peek::Fixed(frame) => frame,
        }
    }

    pub(crate) fn edit(&self) -> Edit {
        let by = self.by;
        match self.trim {
            Trim::Plain { clip, edge } => Edit::Trim {
                clips: vec![clip],
                edge,
                by,
            },
            Trim::Ripple { clip, edge } => Edit::RippleTrim { clip, edge, by },
            Trim::Roll { edit_point } => Edit::Roll { edit_point, by },
            Trim::Slip { clip } => Edit::Slip {
                clips: vec![clip],
                by,
            },
            Trim::Slide { clip } => Edit::Slide { clip, by },
        }
    }
}
