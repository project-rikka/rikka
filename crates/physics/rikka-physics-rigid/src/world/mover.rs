use rikka_physics_collision::{Material, Mover, MoverPlane};
use rikka_physics_math::Vec3;

use super::RigidWorld;
use crate::{BodyId, RayHit, ShapeId, ShapeQuery};

impl RigidWorld {
    pub fn collide_mover<'f>(
        &self,
        mover: &Mover,
        filter: impl Into<ShapeQuery<'f>>,
        mut visit: impl FnMut(ShapeId, &MoverPlane) -> bool,
    ) {
        let tolerance = self.config.tolerance;
        self.with_query(filter.into(), |world, filter| {
            world.collide_mover(mover, filter, tolerance, |collider, plane| {
                visit(self.shape_id(collider), plane)
            });
        });
    }

    pub fn cast_mover<'f>(
        &self,
        mover: &Mover,
        translation: Vec3,
        filter: impl Into<ShapeQuery<'f>>,
    ) -> Option<RayHit> {
        self.with_query(filter.into(), |world, filter| {
            world
                .cast_mover(mover, translation, filter)
                .map(|hit| self.ray_hit(&hit))
        })
    }
}

impl RigidWorld {
    pub(crate) fn material_at(&self, shape: ShapeId, index: u32) -> Option<Material> {
        let shape = self.shape_index(shape).ok()?;
        Some(*self.colliders[shape as usize].material(index))
    }

    pub(crate) fn point_velocity(&self, body: BodyId, point: Vec3) -> Vec3 {
        let Ok(index) = self.body_index(body) else {
            return Vec3::ZERO;
        };
        let state = self.state_of(index);
        state.v + state.w.cross(point - self.pose_of(index).center)
    }

    pub(crate) fn point_response(
        &self,
        body: BodyId,
        point: Vec3,
        normal: Vec3,
    ) -> Option<(f32, Vec3)> {
        let index = self.body_index(body).ok()?;
        let record = self.bodies.get(index);
        let set = self.sets.get(record.set);
        let local = record.local as usize;
        let motion = &set.motion[local];
        let rn = (point - set.poses[local].center).cross(normal);
        let effective = motion.inv_mass + rn.dot(motion.inv_inertia.mul(rn));
        Some((effective, self.point_velocity(body, point)))
    }
}
