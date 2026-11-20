use std::ops::Range;

use crate::clip::{Clip, ClipId};
use crate::height::Height;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Switch {
    Shown,
    Locked,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub(crate) visible: bool,
    pub(crate) locked: bool,
    pub(crate) height: Option<Height>,
    pub(crate) clips: Vec<Clip>,
}

impl Default for Layer {
    fn default() -> Layer {
        Layer {
            visible: true,
            locked: false,
            height: None,
            clips: Vec::new(),
        }
    }
}

impl Layer {
    #[must_use]
    pub fn is_shown(&self) -> bool {
        self.visible
    }

    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    #[must_use]
    pub fn height(&self) -> Option<Height> {
        self.height
    }

    #[must_use]
    pub fn clips(&self) -> &[Clip] {
        &self.clips
    }

    #[must_use]
    pub fn end(&self) -> i64 {
        self.clips.last().map_or(0, Clip::end)
    }

    #[must_use]
    pub fn clip_at(&self, frame: i64) -> Option<&Clip> {
        let after = self.clips.partition_point(|clip| clip.start <= frame);
        after
            .checked_sub(1)
            .map(|at| &self.clips[at])
            .filter(|clip| clip.covers(frame))
    }

    #[must_use]
    pub fn clips_in(&self, range: Range<i64>) -> &[Clip] {
        let first = self.clips.partition_point(|clip| clip.end() <= range.start);
        let past = self.clips.partition_point(|clip| clip.start < range.end);
        &self.clips[first..past.max(first)]
    }

    #[must_use]
    pub fn find(&self, id: ClipId) -> Option<&Clip> {
        self.clips.iter().find(|clip| clip.id == id)
    }

    #[must_use]
    pub(crate) fn is_free(&self, range: &Range<i64>) -> bool {
        !self.locked && self.is_empty_over(range)
    }

    pub(crate) fn set(&mut self, switch: Switch, on: bool) {
        match switch {
            Switch::Shown => self.visible = on,
            Switch::Locked => self.locked = on,
        }
    }

    pub(crate) fn find_mut(&mut self, id: ClipId) -> Option<&mut Clip> {
        self.clips.iter_mut().find(|clip| clip.id == id)
    }

    fn position(&self, id: ClipId) -> usize {
        self.clips
            .iter()
            .position(|clip| clip.id == id)
            .unwrap_or(self.clips.len())
    }

    pub(crate) fn previous_end(&self, id: ClipId) -> i64 {
        let at = self.position(id);
        at.checked_sub(1)
            .map_or(0, |before| self.clips[before].end())
    }

    pub(crate) fn next_start(&self, id: ClipId) -> Option<i64> {
        self.clips.get(self.position(id) + 1).map(|clip| clip.start)
    }

    pub(crate) fn take(&mut self, id: ClipId) -> Option<Clip> {
        let at = self.clips.iter().position(|clip| clip.id == id)?;
        Some(self.clips.remove(at))
    }

    pub(crate) fn place(&mut self, clip: Clip) {
        let at = self.clips.partition_point(|other| other.start < clip.start);
        self.clips.insert(at, clip);
    }

    pub(crate) fn carve(&mut self, range: &Range<i64>, mut fresh: impl FnMut() -> ClipId) {
        let mut clips = Vec::with_capacity(self.clips.len() + 1);
        for mut clip in self.clips.drain(..) {
            if !clip.overlaps(range) {
                clips.push(clip);
                continue;
            }
            match (clip.start < range.start, clip.end() > range.end) {
                (true, true) => {
                    let mut right = clip.clone();
                    right.id = fresh();
                    right.cut_head(range.end - clip.start);
                    right.animation_presets.in_preset = None;
                    right.transitions.into = None;
                    clip.cut_tail(clip.end() - range.start);
                    clip.animation_presets.out_preset = None;
                    clip.transitions.out = None;
                    clips.extend([clip, right]);
                }
                (true, false) => {
                    clip.cut_tail(clip.end() - range.start);
                    clips.push(clip);
                }
                (false, true) => {
                    clip.cut_head(range.end - clip.start);
                    clips.push(clip);
                }
                (false, false) => {}
            }
        }
        self.clips = clips;
    }

    pub(crate) fn shift_from(&mut self, frame: i64, by: i64, except: Option<ClipId>) {
        for clip in &mut self.clips {
            if clip.start >= frame && Some(clip.id) != except {
                clip.start += by;
            }
        }
    }

    pub(crate) fn shifts_from(&self, frame: i64, except: Option<ClipId>) -> bool {
        self.clips
            .iter()
            .any(|clip| clip.start >= frame && Some(clip.id) != except)
    }

    pub(crate) fn room_before(&self, frame: i64) -> Option<i64> {
        let moving = self.clips.iter().position(|clip| clip.start >= frame)?;
        let floor = moving
            .checked_sub(1)
            .map_or(0, |before| self.clips[before].end());
        Some(self.clips[moving].start - floor)
    }

    pub(crate) fn is_empty_over(&self, range: &Range<i64>) -> bool {
        self.clips_in(range.clone()).is_empty()
    }
}
