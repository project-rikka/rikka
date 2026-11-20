use animate_body::{Body, Dof, EYES, HumanBone, HumanPose, arc, muscle_of, muscles, muscles_of};
use rikka_model_core::glam::{Quat, Vec3};

const NECK_SHARE: f32 = 0.4;
const HEAD_SHARE: f32 = 0.6;
const EYE_BONES: [HumanBone; 2] = [HumanBone::LeftEye, HumanBone::RightEye];
const FOLLOWERS: [(HumanBone, f32); 2] =
    [(HumanBone::Neck, NECK_SHARE), (HumanBone::Head, HEAD_SHARE)];

pub fn look(body: &Body, pose: &mut HumanPose, target: Vec3, weight: f32, head_follow: f32) {
    let weight = weight.clamp(0.0, 1.0);
    if weight <= 0.0 || !body.has(HumanBone::Head) {
        return;
    }
    let before = *pose;
    let mut aimed = *pose;
    let miss = aim_eyes(body, &mut aimed, target);
    let follow = head_follow.clamp(0.0, 1.0);
    if follow > 0.0 {
        follow_with_head(body, &mut aimed, miss * follow);
        aim_eyes(body, &mut aimed, target);
    }
    let moved = EYES
        .chain(muscles_of(HumanBone::Neck))
        .chain(muscles_of(HumanBone::Head));
    for index in moved {
        pose.muscles[index] =
            before.muscles[index] + (aimed.muscles[index] - before.muscles[index]) * weight;
    }
}

fn aim_eyes(body: &Body, pose: &mut HumanPose, target: Vec3) -> Vec3 {
    let eyes: Vec<HumanBone> = EYE_BONES.into_iter().filter(|eye| body.has(*eye)).collect();
    let worlds = body.fk(pose);
    let head = worlds.joint(HumanBone::Head);
    if eyes.is_empty() {
        let desired = head.rotation.inverse() * (target - head.position);
        return arc(Vec3::Z, desired).to_scaled_axis();
    }
    let mut miss = Vec3::ZERO;
    for eye in &eyes {
        let desired = head.rotation.inverse() * (target - worlds.position(*eye));
        body.set_local_rotation(*eye, arc(Vec3::Z, desired), pose);
        for index in muscles_of(*eye) {
            pose.muscles[index] = muscles()[index].keep(pose.muscles[index], true);
        }
        let looking = body.local_rotation(*eye, pose) * Vec3::Z;
        miss += arc(looking, desired).to_scaled_axis();
    }
    miss / eyes.len() as f32
}

fn follow_with_head(body: &Body, pose: &mut HumanPose, miss: Vec3) {
    let head_local = body.local_rotation(HumanBone::Head, pose);
    for (bone, share) in FOLLOWERS {
        if !body.has(bone) {
            continue;
        }
        let axis = match bone {
            HumanBone::Head => miss,
            _ => head_local * miss,
        };
        let rotation = body.local_rotation(bone, pose);
        let side = muscle_of(bone, Dof::Side).map(|index| (index, pose.muscles[index]));
        body.set_local_rotation(bone, rotation * Quat::from_scaled_axis(axis * share), pose);
        if let Some((index, value)) = side {
            pose.muscles[index] = value;
        }
    }
}
