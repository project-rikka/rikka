use std::ops::Range;

use cut_project::{
    AnimationPresets, Clip, ClipId, Edge, Edit, EditPoint, MoveLimits, Project, Snap, Transition,
};
use rikka_app::prelude::*;
use rikka_timeline::Band;

use super::leveling::{Fading, Leveling, Rekeying};
use super::marking::Marking;
use super::transition_spans::Grip;
use super::trimming::Trimming;
use super::wedges::{self, WedgeSide};
use crate::timeline::rows::Rows;
use crate::timeline::scale::Scale;

const SNAP: f32 = 8.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Grab {
    PendingMove(PendingMove),
    Moving(Moving),
    Trimming(Trimming),
    PresetResize(PresetResize),
    TransitionResize(TransitionResize),
    Banding(Banding),
    Leveling(Leveling),
    Rekeying(Rekeying),
    Fading(Fading),
    Marking(Marking),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TransitionResize {
    at: Point,
    edit_point: EditPoint,
    grip: Grip,
    joined: bool,
    cut: i64,
    original: Transition,
    transition: Transition,
}

impl TransitionResize {
    pub(crate) fn begin(
        at: Point,
        edit_point: EditPoint,
        grip: Grip,
        project: &Project,
    ) -> Option<TransitionResize> {
        let transition_span = project.transition_span(edit_point)?;
        Some(TransitionResize {
            at,
            edit_point: transition_span.edit_point,
            grip,
            joined: transition_span.out.is_some() && transition_span.into.is_some(),
            cut: transition_span.cut,
            original: *transition_span.transition,
            transition: *transition_span.transition,
        })
    }

    pub(crate) fn edit_point(&self) -> EditPoint {
        self.edit_point
    }

    pub(crate) fn span(&self) -> Range<i64> {
        let length = self.transition.length();
        match (self.joined, self.edit_point.edge) {
            (true, _) => self.cut - self.transition.before..self.cut + self.transition.after,
            (false, Edge::Head) => self.cut..self.cut + length,
            (false, Edge::Tail) => self.cut - length..self.cut,
        }
    }

    pub(crate) fn drag(&mut self, to: Point, scale: &Scale) {
        let by = frames(scale, to.x - self.at.x);
        let original = self.original;
        self.transition = match (self.joined, self.grip, self.edit_point.edge) {
            (true, Grip::Start, _) => Transition {
                before: (original.before - by).max(i64::from(original.after == 0)),
                ..original
            },
            (true, Grip::End, _) => Transition {
                after: (original.after + by).max(i64::from(original.before == 0)),
                ..original
            },
            (false, Grip::End, Edge::Head) => original.with_length(original.length() + by),
            (false, Grip::Start, Edge::Tail) => original.with_length(original.length() - by),
            _ => original,
        };
    }

    pub(crate) fn has_changed(&self) -> bool {
        self.transition != self.original
    }

    pub(crate) fn edit(&self) -> Edit {
        Edit::Transitions {
            edit_points: vec![self.edit_point],
            transition: Some(self.transition),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PendingMove {
    pub(crate) at: Point,
    pub(crate) clips: Vec<ClipId>,
    pub(crate) solo: Option<Vec<ClipId>>,
    pub(crate) snap: Option<Snap>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Moving {
    at: Point,
    pub(crate) clips: Vec<ClipId>,
    edges: Vec<i64>,
    limits: MoveLimits,
    snap: Option<Snap>,
    pub(crate) by: i64,
    pub(crate) across: i32,
    pub(crate) snapped: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PresetResize {
    pub(crate) clip: ClipId,
    side: WedgeSide,
    start: i64,
    length: i64,
    original: AnimationPresets,
    pub(crate) animation_presets: AnimationPresets,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Banding {
    pub(crate) band: Band,
    pub(crate) base: Vec<ClipId>,
}

pub(crate) fn snapping(project: &Project, moving: &[ClipId], playhead: i64) -> Snap {
    Snap::new(snap_points(project, moving, playhead))
}

pub(crate) fn snap_points(
    project: &Project,
    moving: &[ClipId],
    playhead: i64,
) -> impl Iterator<Item = i64> {
    let marks = project.marks();
    let ranges: Vec<i64> = project
        .markers()
        .iter()
        .map(|(_, marker)| marker.end())
        .chain(project.marker_frames())
        .collect();
    project
        .edit_frames_except(moving)
        .into_iter()
        .chain([playhead])
        .chain(marks.mark_in)
        .chain(marks.mark_out.map(|out| out + 1))
        .chain(ranges)
}

pub(crate) fn reach(scale: &Scale) -> i64 {
    scale.frames_across(SNAP).round().max(1.0) as i64
}

pub(crate) fn frames(scale: &Scale, pixels: f32) -> i64 {
    scale.frames_across(pixels).round() as i64
}
impl Moving {
    pub(crate) fn begin(pressed: PendingMove, project: &Project) -> Moving {
        let edges = pressed
            .clips
            .iter()
            .filter_map(|id| project.locate(*id))
            .flat_map(|(_, clip)| [clip.start(), clip.end()])
            .collect();
        Moving {
            at: pressed.at,
            edges,
            limits: project.move_limits(&pressed.clips),
            clips: pressed.clips,
            snap: pressed.snap,
            by: 0,
            across: 0,
            snapped: None,
        }
    }

    pub(crate) fn drag(&mut self, to: Point, scale: &Scale, rows: &Rows) {
        let mut by = frames(scale, to.x - self.at.x).max(self.limits.earliest);
        self.snapped = None;
        if let Some(snap) = &self.snap {
            let edges: Vec<i64> = self.edges.iter().map(|edge| edge + by).collect();
            if let Some(pull) = snap.pull(&edges, reach(scale))
                && by + pull >= self.limits.earliest
            {
                by += pull;
                self.snapped = edges
                    .iter()
                    .map(|edge| edge + pull)
                    .find(|point| snap.pull(&[*point], 0) == Some(0));
            }
        }
        self.by = by;
        self.across = rows.across(self.at.y, to.y).max(-self.limits.highest);
    }

    pub(crate) fn has_moved(&self) -> bool {
        self.by != 0 || self.across != 0
    }
}

impl PresetResize {
    pub(crate) fn begin(clip: &Clip, side: WedgeSide) -> PresetResize {
        PresetResize {
            clip: clip.id(),
            side,
            start: clip.start(),
            length: clip.length(),
            original: *clip.animation_presets(),
            animation_presets: *clip.animation_presets(),
        }
    }

    pub(crate) fn drag(&mut self, to: Point, scale: &Scale) {
        let frame = scale.frame_at(to.x).round() as i64;
        let requested = match self.side {
            WedgeSide::In => frame - self.start,
            WedgeSide::Out => self.start + self.length - frame,
        };
        self.animation_presets = wedges::resized(&self.original, self.side, self.length, requested);
    }

    pub(crate) fn has_changed(&self) -> bool {
        self.animation_presets != self.original
    }
}
