use std::ops::Range;
use std::sync::LazyLock;

use rikka_model_core::HumanBone;
use rikka_model_core::glam::Vec3;

pub const MUSCLE_COUNT: usize = 95;

pub const TORSO: Range<usize> = 0..9;
pub const HEAD: Range<usize> = 9..17;
pub const EYES: Range<usize> = 17..21;
pub const LEFT_ARM: Range<usize> = 21..30;
pub const RIGHT_ARM: Range<usize> = 30..39;
pub const LEFT_FINGERS: Range<usize> = 39..59;
pub const RIGHT_FINGERS: Range<usize> = 59..79;
pub const LEFT_LEG: Range<usize> = 79..87;
pub const RIGHT_LEG: Range<usize> = 87..95;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dof {
    Bend,
    Side,
    Twist,
}

const LIMIT: f32 = 1.0;
const STRETCH: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Muscle {
    pub bone: HumanBone,
    pub dof: Dof,
    pub axis: Vec3,
    pub min: f32,
    pub max: f32,
    lowest: f32,
    highest: f32,
}

impl Muscle {
    #[must_use]
    pub fn angle(&self, value: f32) -> f32 {
        match value >= 0.0 {
            true => value * self.max,
            false => -value * self.min,
        }
    }

    #[must_use]
    pub fn value(&self, angle: f32) -> f32 {
        match angle >= 0.0 {
            true if self.max > 0.0 => angle / self.max,
            false if self.min < 0.0 => -angle / self.min,
            _ => 0.0,
        }
    }

    #[must_use]
    pub fn bounds(&self, limits: bool) -> (f32, f32) {
        let stretch = match limits {
            true => LIMIT,
            false => STRETCH,
        };
        (
            self.value(self.lowest) * stretch,
            self.value(self.highest) * stretch,
        )
    }

    #[must_use]
    pub fn bounds_around(&self, limits: bool, current: f32) -> (f32, f32) {
        let (low, high) = self.bounds(limits);
        (low.min(current), high.max(current))
    }

    #[must_use]
    pub fn angles(&self, limits: bool) -> (f32, f32) {
        let (low, high) = self.bounds(limits);
        (self.angle(low), self.angle(high))
    }

    #[must_use]
    pub fn keep(&self, value: f32, limits: bool) -> f32 {
        let (low, high) = self.bounds(limits);
        value.clamp(low, high)
    }
}

struct Row(HumanBone, Dof, Vec3, f32, f32);

struct Reach(HumanBone, Dof, f32, f32);

const X: Vec3 = Vec3::X;
const Y: Vec3 = Vec3::Y;
const Z: Vec3 = Vec3::Z;
const NX: Vec3 = Vec3::NEG_X;
const NY: Vec3 = Vec3::NEG_Y;
const NZ: Vec3 = Vec3::NEG_Z;

const CENTRE: [Row; 21] = [
    Row(HumanBone::Spine, Dof::Bend, X, -40.0, 40.0),
    Row(HumanBone::Spine, Dof::Side, NZ, -40.0, 40.0),
    Row(HumanBone::Spine, Dof::Twist, Y, -40.0, 40.0),
    Row(HumanBone::Chest, Dof::Bend, X, -40.0, 40.0),
    Row(HumanBone::Chest, Dof::Side, NZ, -40.0, 40.0),
    Row(HumanBone::Chest, Dof::Twist, Y, -40.0, 40.0),
    Row(HumanBone::UpperChest, Dof::Bend, X, -20.0, 20.0),
    Row(HumanBone::UpperChest, Dof::Side, NZ, -20.0, 20.0),
    Row(HumanBone::UpperChest, Dof::Twist, Y, -20.0, 20.0),
    Row(HumanBone::Neck, Dof::Bend, X, -40.0, 40.0),
    Row(HumanBone::Neck, Dof::Side, NZ, -40.0, 40.0),
    Row(HumanBone::Neck, Dof::Twist, Y, -40.0, 40.0),
    Row(HumanBone::Head, Dof::Bend, X, -40.0, 40.0),
    Row(HumanBone::Head, Dof::Side, NZ, -40.0, 40.0),
    Row(HumanBone::Head, Dof::Twist, Y, -40.0, 40.0),
    Row(HumanBone::Jaw, Dof::Bend, X, -10.0, 10.0),
    Row(HumanBone::Jaw, Dof::Side, Y, -10.0, 10.0),
    Row(HumanBone::LeftEye, Dof::Bend, NX, -10.0, 15.0),
    Row(HumanBone::LeftEye, Dof::Side, NY, -20.0, 20.0),
    Row(HumanBone::RightEye, Dof::Bend, NX, -10.0, 15.0),
    Row(HumanBone::RightEye, Dof::Side, Y, -20.0, 20.0),
];

const LEFT_ARM_ROWS: [Row; 9] = [
    Row(HumanBone::LeftShoulder, Dof::Bend, Z, -15.0, 30.0),
    Row(HumanBone::LeftShoulder, Dof::Side, NY, -15.0, 15.0),
    Row(HumanBone::LeftUpperArm, Dof::Bend, Z, -60.0, 100.0),
    Row(HumanBone::LeftUpperArm, Dof::Side, NY, -100.0, 100.0),
    Row(HumanBone::LeftUpperArm, Dof::Twist, X, -90.0, 90.0),
    Row(HumanBone::LeftLowerArm, Dof::Bend, NY, -5.0, 150.0),
    Row(HumanBone::LeftLowerArm, Dof::Twist, X, -90.0, 90.0),
    Row(HumanBone::LeftHand, Dof::Bend, Z, -80.0, 80.0),
    Row(HumanBone::LeftHand, Dof::Side, NY, -40.0, 40.0),
];

const LEFT_FINGER_ROWS: [Row; 20] = [
    Row(HumanBone::LeftThumbMetacarpal, Dof::Bend, NZ, -20.0, 40.0),
    Row(HumanBone::LeftThumbMetacarpal, Dof::Side, NY, -25.0, 25.0),
    Row(HumanBone::LeftThumbProximal, Dof::Bend, NZ, -10.0, 60.0),
    Row(HumanBone::LeftThumbDistal, Dof::Bend, NZ, -10.0, 80.0),
    Row(HumanBone::LeftIndexProximal, Dof::Bend, NZ, -20.0, 90.0),
    Row(HumanBone::LeftIndexProximal, Dof::Side, NY, -20.0, 20.0),
    Row(
        HumanBone::LeftIndexIntermediate,
        Dof::Bend,
        NZ,
        -10.0,
        100.0,
    ),
    Row(HumanBone::LeftIndexDistal, Dof::Bend, NZ, -10.0, 80.0),
    Row(HumanBone::LeftMiddleProximal, Dof::Bend, NZ, -20.0, 90.0),
    Row(HumanBone::LeftMiddleProximal, Dof::Side, Y, -20.0, 20.0),
    Row(
        HumanBone::LeftMiddleIntermediate,
        Dof::Bend,
        NZ,
        -10.0,
        100.0,
    ),
    Row(HumanBone::LeftMiddleDistal, Dof::Bend, NZ, -10.0, 80.0),
    Row(HumanBone::LeftRingProximal, Dof::Bend, NZ, -20.0, 90.0),
    Row(HumanBone::LeftRingProximal, Dof::Side, Y, -20.0, 20.0),
    Row(HumanBone::LeftRingIntermediate, Dof::Bend, NZ, -10.0, 100.0),
    Row(HumanBone::LeftRingDistal, Dof::Bend, NZ, -10.0, 80.0),
    Row(HumanBone::LeftLittleProximal, Dof::Bend, NZ, -20.0, 90.0),
    Row(HumanBone::LeftLittleProximal, Dof::Side, Y, -20.0, 20.0),
    Row(
        HumanBone::LeftLittleIntermediate,
        Dof::Bend,
        NZ,
        -10.0,
        100.0,
    ),
    Row(HumanBone::LeftLittleDistal, Dof::Bend, NZ, -10.0, 80.0),
];

const LEFT_LEG_ROWS: [Row; 8] = [
    Row(HumanBone::LeftUpperLeg, Dof::Bend, NX, -50.0, 170.0),
    Row(HumanBone::LeftUpperLeg, Dof::Side, Z, -60.0, 170.0),
    Row(HumanBone::LeftUpperLeg, Dof::Twist, Y, -60.0, 60.0),
    Row(HumanBone::LeftLowerLeg, Dof::Bend, X, -5.0, 150.0),
    Row(HumanBone::LeftLowerLeg, Dof::Twist, Y, -90.0, 90.0),
    Row(HumanBone::LeftFoot, Dof::Bend, NX, -50.0, 50.0),
    Row(HumanBone::LeftFoot, Dof::Side, Y, -30.0, 30.0),
    Row(HumanBone::LeftToes, Dof::Bend, NX, -50.0, 50.0),
];

const REACHES: [Reach; 5] = [
    Reach(HumanBone::LeftUpperArm, Dof::Bend, -110.0, 100.0),
    Reach(HumanBone::LeftEye, Dof::Bend, -30.0, 25.0),
    Reach(HumanBone::LeftEye, Dof::Side, -35.0, 35.0),
    Reach(HumanBone::RightEye, Dof::Bend, -30.0, 25.0),
    Reach(HumanBone::RightEye, Dof::Side, -35.0, 35.0),
];

fn reach_of(row: &Row) -> (f32, f32) {
    REACHES
        .iter()
        .find(|reach| reach.0 == row.0 && reach.1 == row.1)
        .map_or((row.3, row.4), |reach| (reach.2, reach.3))
}

static MUSCLES: LazyLock<Vec<Muscle>> = LazyLock::new(|| {
    let muscle_from_row = |row: &Row| {
        let (lowest, highest) = reach_of(row);
        Muscle {
            bone: row.0,
            dof: row.1,
            axis: row.2,
            min: row.3.to_radians(),
            max: row.4.to_radians(),
            lowest: lowest.to_radians(),
            highest: highest.to_radians(),
        }
    };
    let mirrored = |row: &Row| {
        let muscle = muscle_from_row(row);
        Muscle {
            bone: right_of(muscle.bone),
            axis: Vec3::new(muscle.axis.x, -muscle.axis.y, -muscle.axis.z),
            ..muscle
        }
    };
    let muscles: Vec<Muscle> = CENTRE
        .iter()
        .map(muscle_from_row)
        .chain(LEFT_ARM_ROWS.iter().map(muscle_from_row))
        .chain(LEFT_ARM_ROWS.iter().map(mirrored))
        .chain(LEFT_FINGER_ROWS.iter().map(muscle_from_row))
        .chain(LEFT_FINGER_ROWS.iter().map(mirrored))
        .chain(LEFT_LEG_ROWS.iter().map(muscle_from_row))
        .chain(LEFT_LEG_ROWS.iter().map(mirrored))
        .collect();
    assert_eq!(muscles.len(), MUSCLE_COUNT);
    muscles
});

#[must_use]
pub fn muscles() -> &'static [Muscle] {
    &MUSCLES
}

#[must_use]
pub fn muscles_of(bone: HumanBone) -> Range<usize> {
    let muscles = muscles();
    let start = muscles
        .iter()
        .position(|muscle| muscle.bone == bone)
        .unwrap_or(muscles.len());
    let end = muscles[start..]
        .iter()
        .position(|muscle| muscle.bone != bone)
        .map_or(muscles.len(), |length| start + length);
    start..end
}

#[must_use]
pub fn muscle_of(bone: HumanBone, dof: Dof) -> Option<usize> {
    muscles_of(bone).find(|index| muscles()[*index].dof == dof)
}

#[must_use]
pub fn right_of(bone: HumanBone) -> HumanBone {
    let name = bone.name();
    name.strip_prefix("left")
        .and_then(|rest| HumanBone::from_name(&format!("right{rest}")))
        .unwrap_or(bone)
}
