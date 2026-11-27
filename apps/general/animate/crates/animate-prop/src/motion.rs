use animate_track::Tracks;

use crate::anchor::{Anchor, AnchorKey};
use crate::key::PropKey;
use crate::levels::PropLevels;
use crate::setting::PropSetting;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropMotion {
    pub levels: PropLevels,
    pub anchor: Option<Anchor>,
    tracks: Tracks<PropKey>,
    anchors: Tracks<AnchorKey>,
}

impl PropMotion {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
            && self.anchors.is_empty()
            && self.anchor.is_none()
            && self.levels == PropLevels::default()
    }

    #[must_use]
    pub fn tracks(&self) -> &Tracks<PropKey> {
        &self.tracks
    }

    pub fn tracks_mut(&mut self) -> &mut Tracks<PropKey> {
        &mut self.tracks
    }

    #[must_use]
    pub fn anchors(&self) -> &Tracks<AnchorKey> {
        &self.anchors
    }

    pub fn anchors_mut(&mut self) -> &mut Tracks<AnchorKey> {
        &mut self.anchors
    }

    #[must_use]
    pub fn last_frame(&self) -> u32 {
        self.tracks
            .last_frame()
            .into_iter()
            .chain(self.anchors.last_frame())
            .max()
            .unwrap_or(0)
    }

    #[must_use]
    pub fn value_at(&self, setting: PropSetting, frame: f32) -> f32 {
        self.tracks
            .value_at(&setting, frame)
            .unwrap_or_else(|| self.levels.get(setting))
    }

    #[must_use]
    pub fn levels_at(&self, frame: f32) -> PropLevels {
        let mut levels = self.levels;
        for setting in self.tracks.keyed() {
            levels.set(*setting, self.value_at(*setting, frame));
        }
        levels
    }

    #[must_use]
    pub fn anchor_at(&self, frame: f32) -> Option<&Anchor> {
        let whole = frame.max(0.0).floor() as u32;
        match self.anchors.held_at(&(), whole) {
            Some(key) => key.anchor.as_ref(),
            None => match self.anchors.keys(&()).first() {
                Some(first) => first.anchor.as_ref(),
                None => self.anchor.as_ref(),
            },
        }
    }

    #[must_use]
    pub fn changed_from(&self, before: &PropMotion) -> Option<u32> {
        if self.levels != before.levels || self.anchor != before.anchor {
            return Some(0);
        }
        [
            self.tracks.changed_from(&before.tracks),
            self.anchors.changed_from(&before.anchors),
        ]
        .into_iter()
        .flatten()
        .min()
    }
}
