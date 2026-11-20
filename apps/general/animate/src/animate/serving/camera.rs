use std::sync::Arc;

use animate_assistant::{CameraKey, Placement};
use animate_cast::{Orbit, Shot, Subject};
use animate_edit::{Change, Key};
use rikka_model::mmd::{VmdCameraInterpolation, VmdCurve};
use rikka_tool::Reply;
use serde_json::json;

use crate::animate::Animate;
use crate::animate::subject_motion::SubjectMotion;

pub(crate) fn shot_of(placement: &Placement) -> Shot {
    let (yaw, elevation, distance) = placement.orbit();
    Shot::placed(
        placement.target,
        Orbit {
            yaw,
            elevation,
            roll: placement.roll,
            distance,
            fov: placement.fov,
        },
    )
}

fn camera_interpolation(curve: VmdCurve) -> VmdCameraInterpolation {
    VmdCameraInterpolation {
        x: curve,
        y: curve,
        z: curve,
        rotation: curve,
        distance: curve,
        angle: curve,
    }
}

impl Animate {
    pub(super) fn key_camera(&mut self, keys: &[CameraKey]) -> Reply {
        let subject_motion = self.camera_motion().map_or_else(
            || SubjectMotion::Camera(Arc::default()),
            SubjectMotion::Camera,
        );
        let standing = self.camera_motion().unwrap_or_default();
        let new_keys: Vec<Key> = keys
            .iter()
            .map(|key| {
                let interpolation = match key.curve {
                    Some(curve) => camera_interpolation(curve),
                    None => standing.tracks().key(&(), key.frame).map_or_else(
                        || camera_interpolation(VmdCurve::LINEAR),
                        |frame| frame.interpolation,
                    ),
                };
                Key::Camera(shot_of(&key.placement).keyed(key.frame, interpolation))
            })
            .collect();
        let count = new_keys.len();
        self.commit(Subject::Camera, Change::laying(&subject_motion, new_keys));
        let frames: Vec<u32> = self
            .camera_motion()
            .map(|film| film.keys().iter().map(|key| key.frame).collect())
            .unwrap_or_default();
        Reply::json(&json!({
            "keyed": count,
            "cameraKeys": frames,
            "filming": self.subject == Some(Subject::Camera),
        }))
    }
}
