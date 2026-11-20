use rikka_physics_geometry::{ShapeProxy, Tolerance};
use rikka_physics_math::Vec3;

use super::{QueryHit, QueryWorld};
use crate::{Mover, MoverPlane, QueryFilter, collide_mover};

impl QueryWorld<'_> {
    pub fn collide_mover(
        &self,
        mover: &Mover,
        filter: &QueryFilter,
        tolerance: Tolerance,
        mut visit: impl FnMut(u32, &MoverPlane) -> bool,
    ) {
        let bounds = mover.bounds();
        self.broadphase.query(&bounds, |index| {
            let collider = &self.colliders[index as usize];
            if !self.accepts(index, collider, filter) {
                return true;
            }
            collide_mover(
                &collider.shape,
                self.frame(collider),
                mover,
                tolerance,
                |plane| visit(index, &plane),
            )
        });
    }

    pub fn cast_mover(
        &self,
        mover: &Mover,
        translation: Vec3,
        filter: &QueryFilter,
    ) -> Option<QueryHit> {
        let points = [mover.center1, mover.center2];
        let proxy = ShapeProxy::new(&points, mover.radius);
        let mut best: Option<QueryHit> = None;
        self.sweep_shape(
            &proxy,
            rikka_physics_math::Transform::IDENTITY,
            translation,
            filter,
            mover.radius > 0.0,
            |hit| {
                if hit.fraction > 0.0 && best.is_none_or(|current| hit.fraction < current.fraction)
                {
                    best = Some(*hit);
                }
                best.map_or(1.0, |hit| hit.fraction)
            },
        );
        best
    }
}
