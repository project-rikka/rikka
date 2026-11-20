use animate_track::Tracks;
use rikka_physics_tailor::Wind;

use crate::key::WindKey;
use crate::levels::WindLevels;
use crate::setting::WindSetting;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindMotion {
    pub levels: WindLevels,
    tracks: Tracks<WindKey>,
}

impl WindMotion {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty() && self.levels == WindLevels::default()
    }

    #[must_use]
    pub fn tracks(&self) -> &Tracks<WindKey> {
        &self.tracks
    }

    pub fn tracks_mut(&mut self) -> &mut Tracks<WindKey> {
        &mut self.tracks
    }

    #[must_use]
    pub fn last_frame(&self) -> u32 {
        self.tracks.last_frame().unwrap_or(0)
    }

    #[must_use]
    pub fn value_at(&self, setting: WindSetting, frame: f32) -> f32 {
        self.tracks
            .value_at(&setting, frame)
            .unwrap_or_else(|| self.levels.get(setting))
    }

    #[must_use]
    pub fn levels_at(&self, frame: f32) -> WindLevels {
        let mut levels = self.levels;
        for setting in self.tracks.keyed() {
            levels.set(*setting, self.value_at(*setting, frame));
        }
        levels
    }

    #[must_use]
    pub fn sample(&self, frame: f32) -> Wind {
        self.levels_at(frame).wind()
    }

    #[must_use]
    pub fn changed_from(&self, before: &WindMotion) -> Option<u32> {
        if self.levels != before.levels {
            return Some(0);
        }
        self.tracks.changed_from(&before.tracks)
    }
}
