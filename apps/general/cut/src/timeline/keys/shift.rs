use std::collections::BTreeSet;
use std::ops::RangeInclusive;

use cut_project::{Clip, KeyRef, KeysChange, Project, Snap};
use rikka_app::prelude::*;
use rikka_timeline::KeyShift;

use crate::timeline::lanes::grab::{reach, snap_points};
use crate::timeline::scale::Scale;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Shift {
    at: Point,
    frames: KeyShift<i64>,
    pub(crate) keys: Vec<KeyRef>,
    snap: Option<Snap>,
    pub(crate) duplicate: bool,
    pub(crate) rise: f32,
}

pub(crate) fn snapping(project: &Project, clip: &Clip, keys: &[KeyRef], playhead: i64) -> Snap {
    let moving: BTreeSet<i64> = keys.iter().map(|key| key.at).collect();
    Snap::new(
        snap_points(project, &[], playhead).chain(
            clip.keyed_frames()
                .into_iter()
                .filter(|frame| !moving.contains(frame)),
        ),
    )
}

impl Shift {
    pub(crate) fn begin(
        at: Point,
        anchor: i64,
        keys: Vec<KeyRef>,
        clip: &Clip,
        snap: Option<Snap>,
        duplicate: bool,
    ) -> Shift {
        let earliest = keys.iter().map(|key| key.at).min().unwrap_or(anchor);
        let latest = keys.iter().map(|key| key.at).max().unwrap_or(anchor);
        let reach: RangeInclusive<i64> =
            (clip.start() - earliest).min(0)..=(clip.end() - 1 - latest).max(0);
        Shift {
            at,
            frames: KeyShift::new(at.x, anchor).within(reach),
            keys,
            snap,
            duplicate,
            rise: 0.0,
        }
    }

    pub(crate) fn by(&self) -> i64 {
        self.frames.by()
    }

    pub(crate) fn snapped(&self) -> Option<i64> {
        self.frames.snapped()
    }

    pub(crate) fn drag(&mut self, to: Point, scale: &Scale) {
        let shown = scale.shown();
        let pull = |at: i64| self.snap.as_ref()?.pull(&[at], reach(scale));
        self.frames.drag(to.x, &shown, pull);
        self.rise = to.y - self.at.y;
    }

    pub(crate) fn has_moved(&self) -> bool {
        self.by() != 0 || self.rise != 0.0
    }

    pub(crate) fn change(&self) -> KeysChange {
        match self.duplicate {
            true => KeysChange::Duplicate { by: self.by() },
            false => KeysChange::Move { by: self.by() },
        }
    }

    pub(crate) fn moved(&self) -> Vec<KeyRef> {
        self.keys
            .iter()
            .map(|key| KeyRef::new(key.property.clone(), key.at + self.by()))
            .collect()
    }
}
