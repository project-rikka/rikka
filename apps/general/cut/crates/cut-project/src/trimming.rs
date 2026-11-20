use std::ops::{Range, RangeInclusive};
use std::sync::Arc;

use crate::clip::{Clip, ClipId, Source};
use crate::edit::Applied;
use crate::error::{Error, Result};
use crate::project::{Edge, Project};
use crate::transitions::EditPoint;

const ENDLESS: i64 = i64::MAX / 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Joint {
    pub layer: usize,
    pub at: i64,
    pub out: Option<ClipId>,
    pub into: Option<ClipId>,
}

impl Clip {
    fn head_reach(&self) -> i64 {
        match &self.source {
            Source::Footage { source_in, .. } => (*source_in as f64 / self.speed()).floor() as i64,
            Source::Graphic(_) | Source::Region(_) | Source::Camera(_) => ENDLESS,
        }
    }

    fn tail_reach(&self, media_frames: i64) -> i64 {
        match &self.source {
            Source::Footage { source_in, .. } => {
                ((media_frames - source_in) as f64 / self.speed()).floor() as i64 - self.length
            }
            Source::Graphic(_) | Source::Region(_) | Source::Camera(_) => ENDLESS,
        }
    }
}

impl Project {
    #[must_use]
    pub fn joint(&self, edit_point: EditPoint) -> Option<Joint> {
        let (layer, clip) = self.locate(edit_point.clip)?;
        let clips = &self.layers[layer].clips;
        let index = clips.iter().position(|other| other.id == clip.id)?;
        let (out, into, at) = match edit_point.edge {
            Edge::Head => {
                let before = index
                    .checked_sub(1)
                    .map(|before| &clips[before])
                    .filter(|before| before.end() == clip.start);
                (before.map(Clip::id), Some(clip.id), clip.start)
            }
            Edge::Tail => {
                let after = clips
                    .get(index + 1)
                    .filter(|after| after.start == clip.end());
                (Some(clip.id), after.map(Clip::id), clip.end())
            }
        };
        Some(Joint {
            layer,
            at,
            out,
            into,
        })
    }

    #[must_use]
    pub fn ripple_limits(&self, id: ClipId, edge: Edge) -> RangeInclusive<i64> {
        let Some((layer, clip)) = self.locate(id) else {
            return 0..=0;
        };
        let media = clip.media().map_or(0, |media| self.media_frames(media));
        let (mut least, mut most, point) = match edge {
            Edge::Head => (-clip.head_reach(), clip.length - 1, clip.start),
            Edge::Tail => (1 - clip.length, clip.tail_reach(media), clip.end()),
        };
        let room = self
            .layers
            .iter()
            .enumerate()
            .filter(|(index, other)| *index != layer && !other.locked)
            .filter_map(|(_, other)| other.room_before(point))
            .min()
            .unwrap_or(ENDLESS);
        match edge {
            Edge::Head => most = most.min(room),
            Edge::Tail => least = least.max(-room),
        }
        range_between(least, most)
    }

    #[must_use]
    pub fn roll_limits(&self, edit_point: EditPoint) -> RangeInclusive<i64> {
        let Some(joint) = self.joint(edit_point) else {
            return 0..=0;
        };
        let layer_ref = &self.layers[joint.layer];
        let clip = |id: Option<ClipId>| id.and_then(|id| layer_ref.find(id));
        let (out, into) = (clip(joint.out), clip(joint.into));
        let mut least = -ENDLESS;
        let mut most = ENDLESS;
        if let Some(out) = out {
            least = least.max(1 - out.length);
            let media = out.media().map_or(0, |media| self.media_frames(media));
            most = most.min(out.tail_reach(media));
            if into.is_none()
                && let Some(next) = layer_ref.next_start(out.id)
            {
                most = most.min(next - out.end());
            }
        }
        if let Some(into) = into {
            most = most.min(into.length - 1);
            least = least.max(-into.head_reach());
            if out.is_none() {
                least = least.max(layer_ref.previous_end(into.id) - into.start);
            }
        }
        range_between(least, most)
    }

    #[must_use]
    pub fn slide_limits(&self, id: ClipId) -> RangeInclusive<i64> {
        let Some((layer, clip)) = self.locate(id) else {
            return 0..=0;
        };
        let layer_ref = &self.layers[layer];
        let (before, after) = self.abutting(id);
        let before = before.and_then(|before| layer_ref.find(before));
        let after = after.and_then(|after| layer_ref.find(after));
        let mut least = before.map_or(layer_ref.previous_end(id) - clip.start, |before| {
            1 - before.length
        });
        let mut most = match after {
            Some(after) => after.length - 1,
            None => layer_ref
                .next_start(id)
                .map_or(ENDLESS, |next| next - clip.end()),
        };
        if let Some(before) = before {
            let media = before.media().map_or(0, |media| self.media_frames(media));
            most = most.min(before.tail_reach(media));
        }
        if let Some(after) = after {
            least = least.max(-after.head_reach());
        }
        range_between(least.max(-clip.start), most)
    }

    fn abutting(&self, id: ClipId) -> (Option<ClipId>, Option<ClipId>) {
        let out = self.joint(EditPoint::head(id)).and_then(|joint| joint.out);
        let into = self.joint(EditPoint::tail(id)).and_then(|joint| joint.into);
        (out, into)
    }

    pub(crate) fn ripple_trim(&mut self, id: ClipId, edge: Edge, by: i64) -> Result<Applied> {
        self.check_unlocked(&[id])?;
        if !self.ripple_limits(id, edge).contains(&by) {
            return Err(Error::OutOfReach);
        }
        let (_, clip) = self.locate(id).ok_or(Error::UnknownClip)?;
        let (point, shift) = match edge {
            Edge::Head => (clip.start, -by),
            Edge::Tail => (clip.end(), by),
        };
        self.ripple_from(point, shift, Some(id));
        self.held_mut(id)?.stretching(|clip| match edge {
            Edge::Head => {
                let start = clip.start;
                clip.cut_head(by);
                clip.start = start;
            }
            Edge::Tail => clip.cut_tail(-by),
        });
        Ok(Applied::selecting(vec![id]))
    }

    pub(crate) fn roll(&mut self, edit_point: EditPoint, by: i64) -> Result<Applied> {
        let joint = self.joint(edit_point).ok_or(Error::UnknownClip)?;
        let sides: Vec<ClipId> = joint.out.into_iter().chain(joint.into).collect();
        self.check_unlocked(&sides)?;
        if !self.roll_limits(edit_point).contains(&by) {
            return Err(Error::OutOfReach);
        }
        if let Some(out) = joint.out {
            self.held_mut(out)?.stretching(|clip| clip.cut_tail(-by));
        }
        if let Some(into) = joint.into {
            self.held_mut(into)?.stretching(|clip| clip.cut_head(by));
        }
        Ok(Applied::default())
    }

    pub(crate) fn slide(&mut self, id: ClipId, by: i64) -> Result<Applied> {
        self.check_unlocked(&[id])?;
        if !self.slide_limits(id).contains(&by) {
            return Err(Error::OutOfReach);
        }
        let (layer, _) = self.locate(id).ok_or(Error::UnknownClip)?;
        let (before, after) = self.abutting(id);
        let layer_ref = self.layer_mut(layer);
        if let Some(before) = before.and_then(|before| layer_ref.find_mut(before)) {
            before.stretching(|clip| clip.cut_tail(-by));
        }
        if let Some(after) = after.and_then(|after| layer_ref.find_mut(after)) {
            after.stretching(|clip| clip.cut_head(by));
        }
        if let Some(clip) = layer_ref.find_mut(id) {
            clip.start += by;
        }
        layer_ref.clips.sort_by_key(|clip| clip.start);
        Ok(Applied::selecting(vec![id]))
    }

    pub(crate) fn extract(&mut self, span: &Range<i64>) -> Result<Applied> {
        if span.is_empty() || span.start < 0 {
            return Err(Error::NothingToTrim);
        }
        let mut next = self.next_id;
        for layer in self.layers.iter_mut().filter(|layer| !layer.locked) {
            if layer.is_empty_over(span) {
                continue;
            }
            Arc::make_mut(layer).carve(span, || {
                next += 1;
                ClipId(next - 1)
            });
        }
        self.next_id = next;
        self.ripple_from(span.end, span.start - span.end, None);
        Ok(Applied::selecting(Vec::new()))
    }

    pub(crate) fn ripple_from(&mut self, frame: i64, by: i64, except: Option<ClipId>) {
        for layer in self
            .layers
            .iter_mut()
            .filter(|layer| !layer.locked && layer.shifts_from(frame, except))
        {
            Arc::make_mut(layer).shift_from(frame, by, except);
        }
        self.markers.shift_from(frame, by);
    }
}

fn range_between(least: i64, most: i64) -> RangeInclusive<i64> {
    match least <= most {
        true => least..=most,
        false => 0..=0,
    }
}
