use rikka_physics_math::{Mat3, Transform, Vec3};

use crate::contact::{ContactHeader, ENABLE_RECYCLE, RECYCLE_VALID, TOUCHING};
use crate::{BodyFrame, ContactManifold, rotation_arc};

const ANGULAR_DISTANCE: f32 = 0.992_403_9;

#[inline]
pub(crate) fn try_recycle(
    header: &ContactHeader,
    a: &BodyFrame,
    b: &BodyFrame,
    recycle: f32,
    speculative: f32,
) -> Option<Refresh> {
    if a.fast
        || b.fast
        || header.flags & (RECYCLE_VALID | ENABLE_RECYCLE) != (RECYCLE_VALID | ENABLE_RECYCLE)
    {
        return None;
    }
    let tolerance = if header.flags & TOUCHING != 0 {
        recycle
    } else {
        recycle.min(speculative)
    };
    let angle_a = a.transform.q.dot(header.cached_rotation_a);
    let angle_b = b.transform.q.dot(header.cached_rotation_b);
    let angular = (angle_a * angle_a).min(angle_b * angle_b);
    if angular <= ANGULAR_DISTANCE {
        return None;
    }
    let cached = header.cached_relative;
    let inv_a = a.transform.q.inverse();
    let relative_p = inv_a * (b.transform.p - a.transform.p);
    let distance_squared = relative_p.distance_squared(cached.p);
    if distance_squared >= tolerance * tolerance {
        return None;
    }
    let slack = tolerance - distance_squared.sqrt();
    let relative_q = inv_a * b.transform.q;
    let turn = cached.q.inverse() * relative_q;
    let reach = a.reach.max(b.reach);
    let arc_squared = 4.0 * rotation_arc(Vec3::new(turn.x, turn.y, turn.z), reach).length_squared();
    if arc_squared >= slack * slack {
        return None;
    }
    if header.points == 0 && header.extra.count == 0 {
        return Some(Refresh::EMPTY);
    }
    Some(Refresh {
        delta_a: Mat3::from_quat(a.transform.q * header.cached_rotation_a.conjugate()),
        delta_b: Mat3::from_quat(b.transform.q * header.cached_rotation_b.conjugate()),
        offset: b.center - a.center,
    })
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Refresh {
    delta_a: Mat3,
    delta_b: Mat3,
    offset: Vec3,
}

impl Refresh {
    const EMPTY: Self = Self {
        delta_a: Mat3::IDENTITY,
        delta_b: Mat3::IDENTITY,
        offset: Vec3::ZERO,
    };

    #[inline]
    pub(crate) fn apply(&self, manifold: &mut ContactManifold) {
        let normal = manifold.normal;
        for point in manifold.points_mut() {
            let r_a = self.delta_a * point.anchor_a;
            let r_b = self.delta_b * point.anchor_b;
            point.separation = point.base_separation + (self.offset + r_b - r_a).dot(normal);
            point.normal_velocity = 0.0;
            point.persisted = true;
        }
    }
}

#[inline]
pub(crate) fn cache_pose(header: &mut ContactHeader, a: &Transform, b: &Transform) {
    header.cached_rotation_a = a.q;
    header.cached_rotation_b = b.q;
    header.cached_relative = a.inverse_mul(b);
    header.flags |= RECYCLE_VALID;
}
