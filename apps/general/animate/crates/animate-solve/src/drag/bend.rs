use animate_body::{HumanBone, HumanPose, TORSO, arc, rotation_vector, twist_about};
use rikka_model_core::glam::{Quat, Vec3};

use crate::drag::{DragScope, Start};
use crate::handle::Handle;
use crate::placing::Placing;

const PASSES: usize = 2;

#[derive(Clone, Debug)]
enum Extension {
    Tilt,
    Torso(Vec<HumanBone>),
}

#[derive(Clone, Debug)]
pub(super) struct Bend {
    bones: Vec<HumanBone>,
    extension: Extension,
}

fn spread(start: &Start<'_>, bones: &[HumanBone], pose: HumanPose, target: Vec3) -> HumanPose {
    let Some(first) = bones.first() else {
        return pose;
    };
    let body = start.body;
    let mut placing = Placing::new(body, pose, start.pose, start.limits);
    let worlds = placing.before();
    let pivot = worlds.position(*first);
    let whole = rotation_vector(arc(
        start.handle.point(body, worlds) - pivot,
        target - pivot,
    ));
    let from: Vec<Quat> = bones.iter().map(|bone| worlds.rotation(*bone)).collect();
    let count = bones.len() as f32;
    for (at, (bone, from)) in bones.iter().zip(from).enumerate() {
        let share = Quat::from_scaled_axis(whole * ((at + 1) as f32 / count));
        placing.swing(*bone, share * from);
    }
    placing.finish()
}

fn tilt(start: &Start<'_>, mut pose: HumanPose, target: Vec3) -> HumanPose {
    let body = start.body;
    let worlds = body.fk(&pose);
    let hips = worlds.position(HumanBone::Hips);
    let turn = arc(start.handle.point(body, &worlds) - hips, target - hips);
    let yaw = Quat::from_axis_angle(Vec3::Y, twist_about(turn, Vec3::Y));
    pose.root_rot = (turn * yaw.inverse() * pose.root_rot).normalize();
    pose
}

impl Bend {
    pub(super) fn new(start: &Start<'_>) -> Self {
        let body = start.body;
        let extension = match start.handle {
            Handle::Head => Extension::Torso(Handle::UpperBody.turned(body)),
            _ => Extension::Tilt,
        };
        Self {
            bones: start.handle.turned(body),
            extension,
        }
    }

    pub(super) fn solve(
        &self,
        start: &Start<'_>,
        mut pose: HumanPose,
        target: Vec3,
        scope: DragScope,
    ) -> HumanPose {
        match &self.extension {
            Extension::Tilt => pose.root_rot = start.pose.root_rot,
            Extension::Torso(_) => pose.muscles[TORSO].copy_from_slice(&start.pose.muscles[TORSO]),
        }
        for _ in 0..PASSES {
            pose = spread(start, &self.bones, pose, target);
        }
        match (scope, &self.extension) {
            (DragScope::Limb, _) => pose,
            (DragScope::FullBody, Extension::Tilt) => tilt(start, pose, target),
            (DragScope::FullBody, Extension::Torso(bones)) => {
                (0..PASSES).fold(pose, |pose, _| spread(start, bones, pose, target))
            }
        }
    }
}
