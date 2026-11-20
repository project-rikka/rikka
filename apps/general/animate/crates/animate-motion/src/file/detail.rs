use animate_track::Tracks;
use rikka_mmd::{
    VmdBoneFrame, VmdCameraFrame, VmdCameraInterpolation, VmdCurve, VmdInterpolation,
    VmdPropertyFrame,
};
use rikka_model_core::glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::morphs::MorphKey;
use crate::take::Take;

use super::check::{finite, sound, unit};
use super::tracks_of;

type CurveBytes = [u8; 4];

pub(super) const CAMERA: &str = "camera";

const SWITCHES: &str = "switches";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DetailFile {
    bones: Vec<BoneFile>,
    properties: Vec<PropertyFile>,
}

impl DetailFile {
    pub(super) fn from_take(take: &Take) -> Self {
        Self {
            bones: take.bones.all_keys().map(BoneFile::from_frame).collect(),
            properties: take
                .switches
                .all_keys()
                .map(PropertyFile::from_frame)
                .collect(),
        }
    }

    pub(super) fn tracks(self) -> Result<(Tracks<VmdBoneFrame>, Tracks<VmdPropertyFrame>)> {
        let bones = tracks_of(self.bones.into_iter().map(BoneFile::frame), |key| {
            key.bone.clone()
        })?;
        let switches = tracks_of(
            self.properties.into_iter().map(|file| Ok(file.frame())),
            |_| SWITCHES.to_owned(),
        )?;
        Ok((bones, switches))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BoneFile {
    name: String,
    frame: u32,
    translation: [f32; 3],
    rotation: [f32; 4],
    curves: [CurveBytes; 4],
    physics_off: bool,
}

impl BoneFile {
    fn from_frame(frame: &VmdBoneFrame) -> Self {
        let curves = &frame.interpolation;
        Self {
            name: frame.bone.clone(),
            frame: frame.frame,
            translation: frame.translation.to_array(),
            rotation: frame.rotation.to_array(),
            curves: [curves.x, curves.y, curves.z, curves.rotation].map(VmdCurve::to_bytes),
            physics_off: frame.physics_off,
        }
    }

    fn frame(self) -> Result<VmdBoneFrame> {
        sound(
            &self.name,
            self.frame,
            finite(&self.translation) && unit(self.rotation),
        )?;
        let [x, y, z, rotation] = self.curves.map(VmdCurve::from_bytes);
        Ok(VmdBoneFrame {
            bone: self.name,
            frame: self.frame,
            translation: Vec3::from_array(self.translation),
            rotation: Quat::from_array(self.rotation),
            interpolation: VmdInterpolation { x, y, z, rotation },
            physics_off: self.physics_off,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MorphFile {
    name: String,
    frame: u32,
    weight: f32,
    curve: CurveBytes,
}

impl MorphFile {
    pub(super) fn from_key(key: &MorphKey) -> Self {
        Self {
            name: key.morph.clone(),
            frame: key.frame,
            weight: key.weight,
            curve: key.curve.to_bytes(),
        }
    }

    pub(super) fn key(self) -> Result<MorphKey> {
        sound(&self.name, self.frame, self.weight.is_finite())?;
        Ok(MorphKey {
            morph: self.name,
            frame: self.frame,
            weight: self.weight,
            curve: VmdCurve::from_bytes(self.curve),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct PropertyFile {
    frame: u32,
    visible: bool,
    ik: Vec<SwitchFile>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SwitchFile {
    name: String,
    on: bool,
}

impl PropertyFile {
    fn from_frame(frame: &VmdPropertyFrame) -> Self {
        Self {
            frame: frame.frame,
            visible: frame.visible,
            ik: frame
                .ik
                .iter()
                .map(|(name, on)| SwitchFile {
                    name: name.clone(),
                    on: *on,
                })
                .collect(),
        }
    }

    fn frame(self) -> VmdPropertyFrame {
        VmdPropertyFrame {
            frame: self.frame,
            visible: self.visible,
            ik: self
                .ik
                .into_iter()
                .map(|switch| (switch.name, switch.on))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct CameraKeyFile {
    frame: u32,
    distance: f32,
    target: [f32; 3],
    rotation: [f32; 3],
    curves: [CurveBytes; 6],
    angle: u32,
    perspective: bool,
}

impl CameraKeyFile {
    pub(super) fn from_frame(frame: &VmdCameraFrame) -> Self {
        let curves = &frame.interpolation;
        Self {
            frame: frame.frame,
            distance: frame.distance,
            target: frame.target.to_array(),
            rotation: frame.rotation.to_array(),
            curves: [
                curves.x,
                curves.y,
                curves.z,
                curves.rotation,
                curves.distance,
                curves.angle,
            ]
            .map(VmdCurve::to_bytes),
            angle: frame.angle,
            perspective: frame.perspective,
        }
    }

    pub(super) fn frame(&self) -> Result<VmdCameraFrame> {
        let placed = self.distance.is_finite() && finite(&self.target) && finite(&self.rotation);
        sound(CAMERA, self.frame, placed)?;
        let [x, y, z, rotation, distance, angle] = self.curves.map(VmdCurve::from_bytes);
        Ok(VmdCameraFrame {
            frame: self.frame,
            distance: self.distance,
            target: Vec3::from_array(self.target),
            rotation: Vec3::from_array(self.rotation),
            interpolation: VmdCameraInterpolation {
                x,
                y,
                z,
                rotation,
                distance,
                angle,
            },
            angle: self.angle,
            perspective: self.perspective,
        })
    }
}
