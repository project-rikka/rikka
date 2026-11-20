use rikka_physics_math::{QuatW, Vec3W, Wide};

use super::math::{delta_rotation, inverse_mul};
use super::model::JointFrameW;
use crate::solver::lanes::BodyW;

#[derive(Clone, Copy, Debug)]
pub(crate) struct JointGeometryW<W: Wide> {
    pub quat_a: QuatW<W>,
    pub relative: QuatW<W>,
    pub r_a: Vec3W<W>,
    pub r_b: Vec3W<W>,
    pub separation: Vec3W<W>,
}

#[inline(always)]
pub(crate) fn relative_rotation<W: Wide>(quat_a: QuatW<W>, quat_b: QuatW<W>) -> QuatW<W> {
    let flip = quat_a.dot(quat_b).simd_lt(W::default());
    let quat_b = QuatW::select(flip, -quat_b, quat_b);
    inverse_mul(quat_a, quat_b)
}

impl<W: Wide> JointGeometryW<W> {
    #[inline(always)]
    pub(crate) fn new(frame: &JointFrameW<W>, a: &BodyW<W>, b: &BodyW<W>) -> Self {
        let quat_a = a.dq * frame.rotation_a;
        let quat_b = b.dq * frame.rotation_b;
        let r_a = a.dq.rotate(frame.anchor_a);
        let r_b = b.dq.rotate(frame.anchor_b);
        Self {
            quat_a,
            relative: relative_rotation(quat_a, quat_b),
            r_a,
            r_b,
            separation: (b.dp - a.dp) + (r_b - r_a) + frame.delta_center,
        }
    }

    #[inline(always)]
    pub(crate) fn rotation_error(&self, target: QuatW<W>) -> Vec3W<W> {
        -self.quat_a.rotate(delta_rotation(self.relative, target))
    }

    #[inline(always)]
    pub(crate) fn world_axis(&self, local: Vec3W<W>) -> Vec3W<W> {
        self.quat_a.rotate(local)
    }
}
