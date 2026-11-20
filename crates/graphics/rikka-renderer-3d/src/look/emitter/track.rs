use glam::{Mat4, Quat, Vec3, Vec4};

use crate::error::{Error, Result};

pub const MAX_TRACK_KEYS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TrackKey {
    pub time: f32,
    pub translation: Vec3,
    pub rotation: Vec4,
    pub scale: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placement {
    pub translation: Vec3,
    pub rotation: Vec4,
    pub scale: Vec3,
    pub velocity: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmitterTrack {
    keys: Vec<TrackKey>,
}

impl Default for EmitterTrack {
    fn default() -> Self {
        Self::fixed(Mat4::IDENTITY)
    }
}

impl EmitterTrack {
    pub fn fixed(transform: Mat4) -> Self {
        Self {
            keys: vec![key(0.0, transform, None)],
        }
    }

    pub fn keyed(keys: impl IntoIterator<Item = (f32, Mat4)>) -> Result<Self> {
        let mut timed: Vec<(f32, Mat4)> = keys.into_iter().collect();
        if timed.is_empty() || timed.len() > MAX_TRACK_KEYS {
            return Err(Error::Invalid(format!(
                "an emitter track holds 1 to {MAX_TRACK_KEYS} keys, got {}",
                timed.len()
            )));
        }
        if let Some((time, _)) = timed.iter().find(|(time, _)| !time.is_finite()) {
            return Err(Error::Invalid(format!(
                "emitter track key time {time} is not finite"
            )));
        }
        timed.sort_by(|a, b| a.0.total_cmp(&b.0));
        if timed.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(Error::Invalid(
                "emitter track keys need distinct times".into(),
            ));
        }
        let mut keys: Vec<TrackKey> = Vec::with_capacity(timed.len());
        for (time, transform) in timed {
            let previous = keys.last().map(|key| key.rotation);
            keys.push(key(time, transform, previous));
        }
        Ok(Self { keys })
    }

    pub fn placed(&self, transform: Mat4) -> Self {
        let mut keys: Vec<TrackKey> = Vec::with_capacity(self.keys.len());
        for each in &self.keys {
            let previous = keys.last().map(|key| key.rotation);
            keys.push(key(each.time, transform * matrix(each), previous));
        }
        Self { keys }
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn keys(&self) -> impl Iterator<Item = (f32, Mat4)> + '_ {
        self.keys.iter().map(|key| (key.time, matrix(key)))
    }

    pub fn transform_at(&self, time: f32) -> Mat4 {
        let placement = self.placement(time);
        Mat4::from_scale_rotation_translation(
            placement.scale,
            Quat::from_vec4(placement.rotation),
            placement.translation,
        )
    }

    pub fn position_at(&self, time: f32) -> Vec3 {
        self.placement(time).translation
    }

    pub(crate) fn raw(&self) -> &[TrackKey] {
        &self.keys
    }

    pub(crate) fn placement(&self, time: f32) -> Placement {
        let keys = &self.keys;
        let first = keys[0];
        let last = keys[keys.len() - 1];
        let still = |key: TrackKey| Placement {
            translation: key.translation,
            rotation: key.rotation,
            scale: key.scale,
            velocity: Vec3::ZERO,
        };
        if keys.len() == 1 || time <= first.time {
            return still(first);
        }
        if time >= last.time {
            return still(last);
        }
        let mut at = 0;
        while at + 2 < keys.len() && time >= keys[at + 1].time {
            at += 1;
        }
        let from = keys[at];
        let to = keys[at + 1];
        let span = to.time - from.time;
        let mix = (time - from.time) / span;
        let rotation = from.rotation + (to.rotation - from.rotation) * mix;
        Placement {
            translation: from.translation + (to.translation - from.translation) * mix,
            rotation: rotation / rotation.length(),
            scale: from.scale + (to.scale - from.scale) * mix,
            velocity: (to.translation - from.translation) / span,
        }
    }
}

fn key(time: f32, transform: Mat4, previous: Option<Vec4>) -> TrackKey {
    let (scale, rotation, translation) = transform.to_scale_rotation_translation();
    let mut rotation = Vec4::from(rotation.normalize());
    if previous.is_some_and(|previous| previous.dot(rotation) < 0.0) {
        rotation = -rotation;
    }
    TrackKey {
        time,
        translation,
        rotation,
        scale,
    }
}

fn matrix(key: &TrackKey) -> Mat4 {
    Mat4::from_scale_rotation_translation(key.scale, Quat::from_vec4(key.rotation), key.translation)
}

pub(crate) fn rotate(rotation: Vec4, v: Vec3) -> Vec3 {
    let axis = rotation.truncate();
    let t = axis.cross(v) * 2.0;
    v + t * rotation.w + axis.cross(t)
}
