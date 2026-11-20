use rikka_model_core::HumanBone;
use rikka_model_core::glam::Vec3;

use crate::muscle::right_of;

pub(crate) const HUMAN_BONES: usize = 55;

#[must_use]
pub(crate) fn left_of(bone: HumanBone) -> HumanBone {
    bone.name()
        .strip_prefix("right")
        .and_then(|rest| HumanBone::from_name(&format!("left{rest}")))
        .unwrap_or(bone)
}

pub(crate) fn is_right(bone: HumanBone) -> bool {
    bone.name().starts_with("right")
}

fn sided(bones: &'static [HumanBone], right: bool) -> Vec<HumanBone> {
    bones
        .iter()
        .map(|bone| match right {
            true => right_of(*bone),
            false => *bone,
        })
        .collect()
}

pub(crate) fn parents(bone: HumanBone) -> Vec<HumanBone> {
    use HumanBone as B;
    let chain: &'static [HumanBone] = match left_of(bone) {
        B::Hips => &[],
        B::Spine | B::LeftUpperLeg => &[B::Hips],
        B::Chest => &[B::Spine],
        B::UpperChest => &[B::Chest, B::Spine],
        B::Neck | B::LeftShoulder => &[B::UpperChest, B::Chest, B::Spine],
        B::Head => &[B::Neck, B::UpperChest, B::Chest, B::Spine],
        B::LeftEye | B::Jaw => &[B::Head],
        B::LeftLowerLeg => &[B::LeftUpperLeg],
        B::LeftFoot => &[B::LeftLowerLeg],
        B::LeftToes => &[B::LeftFoot],
        B::LeftUpperArm => &[B::LeftShoulder, B::UpperChest, B::Chest, B::Spine],
        B::LeftLowerArm => &[B::LeftUpperArm],
        B::LeftHand => &[B::LeftLowerArm],
        B::LeftThumbMetacarpal
        | B::LeftIndexProximal
        | B::LeftMiddleProximal
        | B::LeftRingProximal
        | B::LeftLittleProximal => &[B::LeftHand],
        B::LeftThumbProximal => &[B::LeftThumbMetacarpal, B::LeftHand],
        B::LeftThumbDistal => &[B::LeftThumbProximal, B::LeftThumbMetacarpal, B::LeftHand],
        B::LeftIndexIntermediate => &[B::LeftIndexProximal, B::LeftHand],
        B::LeftIndexDistal => &[B::LeftIndexIntermediate, B::LeftIndexProximal, B::LeftHand],
        B::LeftMiddleIntermediate => &[B::LeftMiddleProximal, B::LeftHand],
        B::LeftMiddleDistal => &[
            B::LeftMiddleIntermediate,
            B::LeftMiddleProximal,
            B::LeftHand,
        ],
        B::LeftRingIntermediate => &[B::LeftRingProximal, B::LeftHand],
        B::LeftRingDistal => &[B::LeftRingIntermediate, B::LeftRingProximal, B::LeftHand],
        B::LeftLittleIntermediate => &[B::LeftLittleProximal, B::LeftHand],
        B::LeftLittleDistal => &[
            B::LeftLittleIntermediate,
            B::LeftLittleProximal,
            B::LeftHand,
        ],
        _ => &[],
    };
    sided(chain, is_right(bone))
}

pub(crate) fn aims(bone: HumanBone) -> Vec<HumanBone> {
    use HumanBone as B;
    let chain: &'static [HumanBone] = match left_of(bone) {
        B::Spine => &[B::Chest, B::UpperChest, B::Neck, B::Head],
        B::Chest => &[B::UpperChest, B::Neck, B::Head],
        B::UpperChest => &[B::Neck, B::Head],
        B::Neck => &[B::Head],
        B::LeftShoulder => &[B::LeftUpperArm],
        B::LeftUpperArm => &[B::LeftLowerArm],
        B::LeftLowerArm => &[B::LeftHand],
        B::LeftHand => &[
            B::LeftMiddleProximal,
            B::LeftIndexProximal,
            B::LeftRingProximal,
            B::LeftLittleProximal,
        ],
        B::LeftThumbMetacarpal => &[B::LeftThumbProximal],
        B::LeftThumbProximal => &[B::LeftThumbDistal],
        B::LeftIndexProximal => &[B::LeftIndexIntermediate],
        B::LeftIndexIntermediate => &[B::LeftIndexDistal],
        B::LeftMiddleProximal => &[B::LeftMiddleIntermediate],
        B::LeftMiddleIntermediate => &[B::LeftMiddleDistal],
        B::LeftRingProximal => &[B::LeftRingIntermediate],
        B::LeftRingIntermediate => &[B::LeftRingDistal],
        B::LeftLittleProximal => &[B::LeftLittleIntermediate],
        B::LeftLittleIntermediate => &[B::LeftLittleDistal],
        B::LeftUpperLeg => &[B::LeftLowerLeg],
        B::LeftLowerLeg => &[B::LeftFoot],
        B::LeftFoot => &[B::LeftToes],
        _ => &[],
    };
    sided(chain, is_right(bone))
}

pub(crate) fn heading(bone: HumanBone) -> Vec3 {
    use HumanBone as B;
    let thumb = Vec3::new(30f32.to_radians().cos(), 0.0, 30f32.to_radians().sin());
    let left = match left_of(bone) {
        B::Hips | B::Spine | B::Chest | B::UpperChest | B::Neck | B::Head => Vec3::Y,
        B::LeftEye | B::Jaw | B::LeftFoot | B::LeftToes => Vec3::Z,
        B::LeftUpperLeg | B::LeftLowerLeg => Vec3::NEG_Y,
        B::LeftThumbMetacarpal | B::LeftThumbProximal | B::LeftThumbDistal => thumb,
        _ => Vec3::X,
    };
    match is_right(bone) {
        true => Vec3::new(-left.x, left.y, left.z),
        false => left,
    }
}
