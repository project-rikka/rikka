use animate_body::{Body, Dof, HumanBone, HumanPose, Side, Worlds, arc, muscle_of, muscles};
use rikka_model_core::glam::Vec3;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EyeDirection {
    pub across: f32,
    pub up: f32,
}

#[derive(Clone, Copy)]
struct EyeMuscle {
    index: usize,
    sign: f32,
}

impl EyeMuscle {
    fn value(self, share: f32, limits: bool) -> f32 {
        let (low, high) = muscles()[self.index].bounds(limits);
        let desired = share.clamp(-1.0, 1.0) * self.sign;
        match desired >= 0.0 {
            true => desired * high,
            false => -desired * low,
        }
    }

    fn share(self, value: f32, limits: bool) -> f32 {
        let (low, high) = muscles()[self.index].bounds(limits);
        let reach = match value >= 0.0 {
            true => high,
            false => -low,
        };
        match reach > f32::EPSILON {
            true => (value / reach * self.sign).clamp(-1.0, 1.0),
            false => 0.0,
        }
    }
}

fn eye_muscles(eye: Side) -> Option<(EyeMuscle, EyeMuscle)> {
    let bone = eye.eye();
    let across = muscle_of(bone, Dof::Side)?;
    let up = muscle_of(bone, Dof::Bend)?;
    let table = muscles();
    Some((
        EyeMuscle {
            index: across,
            sign: table[across].axis.y.signum(),
        },
        EyeMuscle {
            index: up,
            sign: -table[up].axis.x.signum(),
        },
    ))
}

impl EyeDirection {
    #[must_use]
    pub fn from_pose(pose: &HumanPose, eye: Side, limits: bool) -> Self {
        let Some((across, up)) = eye_muscles(eye) else {
            return Self::default();
        };
        Self {
            across: across.share(pose.muscles[across.index], limits),
            up: up.share(pose.muscles[up.index], limits),
        }
    }

    #[must_use]
    pub fn aim(
        body: &Body,
        worlds: &Worlds,
        eye: Side,
        target: Vec3,
        limits: bool,
    ) -> Option<Self> {
        let bone = eye.eye();
        if !body.has(bone) || !body.has(HumanBone::Head) {
            return None;
        }
        let desired = worlds.rotation(HumanBone::Head).inverse() * (target - worlds.position(bone));
        let mut pose = HumanPose::default();
        body.set_local_rotation(bone, arc(Vec3::Z, desired), &mut pose);
        Some(Self::from_pose(&pose, eye, limits))
    }

    pub fn apply(self, pose: &mut HumanPose, eye: Side, limits: bool) {
        let Some((across, up)) = eye_muscles(eye) else {
            return;
        };
        pose.muscles[across.index] = across.value(self.across, limits);
        pose.muscles[up.index] = up.value(self.up, limits);
    }
}
