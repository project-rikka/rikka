use animate_track::Tracks;
use rikka_mmd::{FRAMES_PER_SECOND, VmdCameraFrame, VmdCameraPose};

use crate::incoming::Merge;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CameraMotion {
    tracks: Tracks<VmdCameraFrame>,
}

impl CameraMotion {
    #[must_use]
    pub fn tracks(&self) -> &Tracks<VmdCameraFrame> {
        &self.tracks
    }

    pub fn tracks_mut(&mut self) -> &mut Tracks<VmdCameraFrame> {
        &mut self.tracks
    }

    #[must_use]
    pub fn keys(&self) -> &[VmdCameraFrame] {
        self.tracks.keys(&())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    #[must_use]
    pub fn last_frame(&self) -> u32 {
        self.tracks.last_frame().unwrap_or(0)
    }

    #[must_use]
    pub fn pose_at(&self, seconds: f32) -> Option<VmdCameraPose> {
        self.tracks.value_at(&(), seconds * FRAMES_PER_SECOND)
    }
}

impl FromIterator<VmdCameraFrame> for CameraMotion {
    fn from_iter<I: IntoIterator<Item = VmdCameraFrame>>(frames: I) -> Self {
        Self {
            tracks: frames.into_iter().collect(),
        }
    }
}

impl Merge for CameraMotion {
    fn merge_into(&mut self, other: Self) {
        self.tracks.merge(other.tracks);
    }
}
