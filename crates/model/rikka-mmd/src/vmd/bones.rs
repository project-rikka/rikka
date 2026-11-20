use rikka_model_core::glam::{Quat, Vec3};
use rustc_hash::FxHashMap;

use super::animation::{bent, turned};
use super::document::{Vmd, VmdBoneFrame};

#[derive(Clone, Debug, Default)]
pub struct VmdBones<'a> {
    tracks: FxHashMap<&'a str, Vec<&'a VmdBoneFrame>>,
}

impl Vmd {
    #[must_use]
    pub fn bone_tracks(&self) -> VmdBones<'_> {
        VmdBones {
            tracks: self.bones_by_name().into_iter().collect(),
        }
    }

    #[must_use]
    pub fn bone_at(&self, bone: &str, frame: f32) -> Option<(Vec3, Quat)> {
        let mut track: Vec<&VmdBoneFrame> =
            self.bones.iter().filter(|key| key.bone == bone).collect();
        track.sort_by_key(|key| key.frame);
        sample(&track, frame)
    }
}

impl VmdBoneFrame {
    #[must_use]
    pub fn toward(&self, to: &VmdBoneFrame, factor: f32) -> (Vec3, Quat) {
        (bent(self, to, factor), turned(self, to, factor))
    }
}

impl<'a> VmdBones<'a> {
    #[must_use]
    pub fn track(&self, bone: &str) -> &[&'a VmdBoneFrame] {
        self.tracks.get(bone).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn key(&self, bone: &str, frame: u32) -> Option<&'a VmdBoneFrame> {
        let track = self.track(bone);
        let at = track.partition_point(|key| key.frame < frame);
        track.get(at).copied().filter(|key| key.frame == frame)
    }

    #[must_use]
    pub fn at(&self, bone: &str, frame: f32) -> Option<(Vec3, Quat)> {
        sample(self.track(bone), frame)
    }
}

fn sample(track: &[&VmdBoneFrame], frame: f32) -> Option<(Vec3, Quat)> {
    (!track.is_empty()).then(|| pose(track, frame))
}

pub(super) fn pose(track: &[&VmdBoneFrame], frame: f32) -> (Vec3, Quat) {
    let next = track.partition_point(|key| key.frame as f32 <= frame);
    let from = track[next.saturating_sub(1)];
    match (next, track.get(next)) {
        (0, _) | (_, None) => (from.translation, from.rotation),
        (_, Some(to)) => {
            let factor = (frame - from.frame as f32) / (to.frame - from.frame) as f32;
            from.toward(to, factor)
        }
    }
}
