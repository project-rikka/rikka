use std::f32::consts::PI;

use rikka_physics_math::{Quat, Transform, Vec3};

use super::drive::Drive;
use super::drive_table::{ANGULAR_POSE, DRIVE_SLOTS, DriveTable, LINEAR_POSE};
use super::motion::{Axis, Motion};
use super::spring::Spring;
use super::swing_twist::{angles, compose};

pub(crate) const LINEAR_BLOCK: u32 = 1;
pub(crate) const LINEAR_ROWS: u32 = 1 << 1;
pub(crate) const ANGULAR_BLOCK: u32 = 1 << 2;
pub(crate) const TWIST: u32 = 1 << 3;
pub(crate) const SWING: u32 = 1 << 4;
pub(crate) const CONE: u32 = 1 << 5;
pub(crate) const LINEAR_DRIVES: u32 = 1 << 6;
pub(crate) const ANGULAR_DRIVES: u32 = 1 << 7;
pub(crate) const POSE_LINEAR: u32 = 1 << 8;
pub(crate) const POSE_ANGULAR: u32 = 1 << 9;
pub(crate) const ANGULAR_SPRINGS: u32 = 1 << 10;
pub(crate) const LINEAR_RANGES: u32 = 1 << 11;
pub(crate) const TWIST_RANGE: u32 = 1 << 12;
pub(crate) const SWING_RANGES: u32 = 1 << 13;
pub(crate) const LINEAR_MOTORS: u32 = 1 << 14;
pub(crate) const ANGULAR_MOTORS: u32 = 1 << 15;

const SWING_X: usize = 3;
const SWING_Y: usize = 4;
const NO_CONE: [f32; 2] = [-1.0, -1.0];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axes {
    lower: [f32; 6],
    upper: [f32; 6],
    targets: [f32; 6],
    velocities: [f32; 6],
    motor_caps: [f32; 6],
    rotation: [f32; 4],
    locked: [f32; 4],
    cone: [f32; 2],
    drives: DriveTable,
    layout: u32,
}

impl Axes {
    pub(crate) fn uniform(motion: Motion) -> Self {
        let (lower, upper) = motion.encode();
        let mut axes = Self {
            lower: [lower; 6],
            upper: [upper; 6],
            targets: [0.0; 6],
            velocities: [0.0; 6],
            motor_caps: [0.0; 6],
            rotation: Quat::IDENTITY.to_array(),
            locked: Quat::IDENTITY.to_array(),
            cone: NO_CONE,
            drives: DriveTable::EMPTY,
            layout: 0,
        };
        axes.relayout();
        axes
    }

    fn bounds(&self, index: usize) -> Option<(f32, f32)> {
        Motion::decode(self.lower[index], self.upper[index]).bounds()
    }

    pub fn motion(&self, axis: Axis) -> Motion {
        Motion::decode(self.lower[axis.index()], self.upper[axis.index()])
    }

    pub(crate) fn set_motion(&mut self, axis: Axis, motion: Motion) {
        (self.lower[axis.index()], self.upper[axis.index()]) = motion.encode();
        if matches!(axis, Axis::SwingX | Axis::SwingY) {
            self.cone = NO_CONE;
        }
        self.relayout();
    }

    pub fn cone(&self) -> Option<(f32, f32)> {
        (self.cone != NO_CONE).then_some((self.cone[0], self.cone[1]))
    }

    pub(crate) fn set_cone(&mut self, angle_x: f32, angle_y: f32) {
        self.cone = [angle_x, angle_y];
        self.relayout();
    }

    fn position(&self) -> Vec3 {
        Vec3::new(self.targets[0], self.targets[1], self.targets[2])
    }

    pub fn target(&self) -> Transform {
        Transform::new(self.position(), Quat::from_array(self.rotation))
    }

    pub(crate) fn set_target(&mut self, target: Transform) {
        self.set_target_translation(target.p);
        self.set_target_rotation(target.q);
    }

    pub(crate) fn set_target_rotation(&mut self, rotation: Quat) {
        self.rotation = rotation.to_array();
        self.targets[3..].copy_from_slice(&angles(rotation).to_array());
    }

    pub(crate) fn set_target_translation(&mut self, translation: Vec3) {
        self.targets[..3].copy_from_slice(&translation.to_array());
    }

    pub fn target_linear_velocity(&self) -> Vec3 {
        Vec3::new(self.velocities[0], self.velocities[1], self.velocities[2])
    }

    pub fn target_angular_velocity(&self) -> Vec3 {
        Vec3::new(self.velocities[3], self.velocities[4], self.velocities[5])
    }

    pub fn target_value(&self, axis: Axis) -> f32 {
        self.targets[axis.index()]
    }

    pub fn target_velocity(&self, axis: Axis) -> f32 {
        self.velocities[axis.index()]
    }

    pub fn drive(&self, axis: Axis) -> Option<Drive> {
        self.drives.get(axis.index())
    }

    pub fn motor_cap(&self, axis: Axis) -> f32 {
        self.motor_caps[axis.index()]
    }

    pub fn linear_pose_drive(&self) -> Option<Drive> {
        self.drives.get(LINEAR_POSE)
    }

    pub fn angular_pose_drive(&self) -> Option<Drive> {
        self.drives.get(ANGULAR_POSE)
    }

    pub(crate) fn set_linear_velocity(&mut self, velocity: Vec3) {
        self.velocities[..3].copy_from_slice(&velocity.to_array());
    }

    pub(crate) fn set_angular_velocity(&mut self, velocity: Vec3) {
        self.velocities[3..].copy_from_slice(&velocity.to_array());
    }

    pub(crate) fn lower(&self) -> &[f32; 6] {
        &self.lower
    }

    pub(crate) fn upper(&self) -> &[f32; 6] {
        &self.upper
    }

    pub(crate) fn targets(&self) -> &[f32; 6] {
        &self.targets
    }

    pub(crate) fn velocities(&self) -> &[f32; 6] {
        &self.velocities
    }

    pub(crate) fn rotation(&self) -> &[f32; 4] {
        &self.rotation
    }

    pub(crate) fn drives(&self) -> &DriveTable {
        &self.drives
    }

    pub(crate) fn set_drive(&mut self, axis: Axis, drive: Drive) {
        self.drives.set(axis.index(), Some(drive));
        self.relayout();
    }

    pub(crate) fn set_linear_pose(&mut self, drive: Drive) {
        self.drives.set(LINEAR_POSE, Some(drive));
        self.relayout();
    }

    pub(crate) fn set_angular_pose(&mut self, drive: Drive) {
        self.drives.set(ANGULAR_POSE, Some(drive));
        self.relayout();
    }

    pub(crate) fn update_linear_pose(&mut self, change: impl FnOnce(&mut Drive)) {
        self.drives.update(LINEAR_POSE, Drive::velocity(), change);
        self.relayout();
    }

    pub(crate) fn update_angular_pose(&mut self, change: impl FnOnce(&mut Drive)) {
        self.drives.update(ANGULAR_POSE, Drive::velocity(), change);
        self.relayout();
    }

    pub(crate) fn set_spring(&mut self, axis: Axis, spring: Spring) {
        self.drives
            .update(axis.index(), Drive::spring(spring), |drive| {
                drive.spring = Some(spring)
            });
        self.relayout();
    }

    pub(crate) fn set_max_effort(&mut self, axis: Axis, effort: f32) {
        self.drives
            .update(axis.index(), Drive::velocity(), |drive| {
                drive.max_effort = effort
            });
        self.relayout();
    }

    pub(crate) fn set_motor(&mut self, axis: Axis, max_effort: f32) {
        self.motor_caps[axis.index()] = max_effort;
        self.relayout();
    }

    pub(crate) fn motor_caps(&self) -> &[f32; 6] {
        &self.motor_caps
    }

    pub(crate) fn set_velocity(&mut self, axis: Axis, velocity: f32) {
        self.velocities[axis.index()] = velocity;
    }

    pub(crate) fn layout(&self) -> u32 {
        self.layout
    }

    fn relayout(&mut self) {
        let locked = |index: usize| {
            self.bounds(index)
                .is_some_and(|(lower, upper)| lower == upper)
        };
        let free = |index: usize| self.bounds(index).is_none();
        let ranged = |index: usize| {
            self.bounds(index)
                .is_some_and(|(lower, upper)| lower < upper)
        };
        let mut layout = 0;
        if (0..3).all(locked) {
            layout |= LINEAR_BLOCK;
        } else if !(0..3).all(free) {
            layout |= LINEAR_ROWS;
            if (0..3).any(ranged) {
                layout |= LINEAR_RANGES;
            }
        }
        if self.cone().is_none() && (3..6).all(locked) {
            layout |= ANGULAR_BLOCK;
        } else {
            if !free(5) {
                layout |= TWIST;
                if ranged(5) {
                    layout |= TWIST_RANGE;
                }
            }
            if self.cone().is_some() {
                layout |= CONE;
            } else if !free(SWING_X) || !free(SWING_Y) {
                layout |= SWING;
                if ranged(SWING_X) || ranged(SWING_Y) {
                    layout |= SWING_RANGES;
                }
            }
        }
        if (0..3).any(|slot| self.drives.is_set(slot)) {
            layout |= LINEAR_DRIVES;
        }
        if (3..6).any(|slot| self.drives.is_set(slot)) {
            layout |= ANGULAR_DRIVES;
        }
        if (3..6).any(|slot| self.drives.has_spring(slot)) {
            layout |= ANGULAR_SPRINGS;
        }
        if self.motor_caps[..3].iter().any(|&cap| cap > 0.0) {
            layout |= LINEAR_MOTORS;
        }
        if self.motor_caps[3..].iter().any(|&cap| cap > 0.0) {
            layout |= ANGULAR_MOTORS;
        }
        if self.drives.is_set(LINEAR_POSE) {
            layout |= POSE_LINEAR;
        }
        if self.drives.is_set(ANGULAR_POSE) {
            layout |= POSE_ANGULAR;
        }
        self.layout = layout;
        let lower = |index: usize| self.bounds(index).map_or(0.0, |(lower, _)| lower);
        self.locked = compose(Vec3::new(lower(3), lower(4), lower(5))).to_array();
    }

    pub(crate) fn is_valid(&self) -> bool {
        let motion = (0..6).all(|index| {
            Motion::decode(self.lower[index], self.upper[index]).is_free()
                || (self.lower[index] <= self.upper[index]
                    && self.lower[index].is_finite()
                    && self.upper[index].is_finite()
                    && (index < 3 || (-PI <= self.lower[index] && self.upper[index] <= PI)))
        });
        let cone = self.cone().is_none_or(|(x, y)| {
            x.is_finite() && y.is_finite() && (0.0..PI).contains(&x) && (0.0..PI).contains(&y)
        });
        let drives =
            (0..DRIVE_SLOTS).all(|slot| self.drives.get(slot).is_none_or(|drive| drive.is_valid()));
        let motors = self.motor_caps.iter().all(|&cap| cap >= 0.0);
        motion
            && cone
            && drives
            && motors
            && self.target().is_finite()
            && self.target().q.is_normalized()
            && self.velocities.iter().all(|velocity| velocity.is_finite())
    }

    pub(crate) fn locked(&self) -> &[f32; 4] {
        &self.locked
    }
}

impl rikka_file_format::Codec for Axes {
    type Error = crate::Error;

    fn encode(&self, out: &mut rikka_file_format::Writer<'_>) {
        out.put_all(&self.lower);
        out.put_all(&self.upper);
        out.put_all(&self.cone);
        for slot in 0..DRIVE_SLOTS {
            out.put(&self.drives.get(slot));
        }
        out.put(&self.position());
        out.put_all(&self.rotation);
        out.put(&self.target_linear_velocity());
        out.put(&self.target_angular_velocity());
        out.put_all(&self.motor_caps);
    }

    fn decode(input: &mut rikka_file_format::Reader<'_>) -> crate::Result<Self> {
        let mut axes = Self::uniform(Motion::Free);
        for value in axes.lower.iter_mut().chain(&mut axes.upper) {
            *value = input.get::<f32>()?;
        }
        for value in &mut axes.cone {
            *value = input.get::<f32>()?;
        }
        for slot in 0..DRIVE_SLOTS {
            axes.drives.set(slot, input.get::<Option<Drive>>()?);
        }
        let position = input.get()?;
        let mut rotation = [0.0f32; 4];
        for value in &mut rotation {
            *value = input.get::<f32>()?;
        }
        axes.set_target(Transform::new(position, Quat::from_array(rotation)));
        axes.set_linear_velocity(input.get()?);
        axes.set_angular_velocity(input.get()?);
        for cap in &mut axes.motor_caps {
            *cap = input.get::<f32>()?;
        }
        axes.relayout();
        Ok(axes)
    }
}
