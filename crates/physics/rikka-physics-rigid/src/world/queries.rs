use rikka_physics_collision::{Material, PointHit, QueryHit, QueryWorld, Ray};
use rikka_physics_geometry::ShapeProxy;
use rikka_physics_math::{Aabb, Transform, Vec3};

use super::RigidWorld;
use crate::{BodyId, ShapeId, ShapeQuery};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub shape: ShapeId,
    pub body: BodyId,
    pub point: Vec3,
    pub normal: Vec3,
    pub fraction: f32,
    pub triangle: u32,
    pub child: u32,
    pub material: Material,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClosestPoint {
    pub shape: ShapeId,
    pub body: BodyId,
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
    pub triangle: u32,
    pub child: u32,
    pub material: Material,
}

impl RigidWorld {
    pub(crate) fn with_query<R>(
        &self,
        query: ShapeQuery<'_>,
        run: impl FnOnce(&QueryWorld<'_>, &rikka_physics_collision::QueryFilter) -> R,
    ) -> R {
        let adapter = |collider: u32| {
            query
                .custom
                .is_none_or(|custom| custom(self.shape_id(collider)))
        };
        let world = QueryWorld {
            broadphase: &self.broadphase,
            colliders: &self.colliders,
            frames: &self.frames,
            custom: query.custom.map(|_| &adapter as &dyn Fn(u32) -> bool),
        };
        run(&world, &query.filter)
    }

    pub(crate) fn ray_hit(&self, hit: &QueryHit) -> RayHit {
        RayHit {
            shape: self.shape_id(hit.collider),
            body: self.body_id(self.colliders[hit.collider as usize].owner),
            point: hit.point,
            normal: hit.normal,
            fraction: hit.fraction,
            triangle: hit.triangle,
            child: hit.child,
            material: hit.material,
        }
    }

    pub fn cast_ray_closest<'f>(
        &self,
        origin: Vec3,
        translation: Vec3,
        filter: impl Into<ShapeQuery<'f>>,
    ) -> Option<RayHit> {
        self.with_query(filter.into(), |world, filter| {
            world
                .cast_ray_closest(origin, translation, filter)
                .map(|hit| self.ray_hit(&hit))
        })
    }

    pub fn cast_ray<'f>(
        &self,
        origin: Vec3,
        translation: Vec3,
        filter: impl Into<ShapeQuery<'f>>,
        mut visit: impl FnMut(&RayHit) -> f32,
    ) {
        self.with_query(filter.into(), |world, filter| {
            world.cast_ray(origin, translation, filter, |hit| visit(&self.ray_hit(hit)));
        });
    }

    pub fn cast_rays_closest<'f>(
        &self,
        rays: &[Ray],
        filter: impl Into<ShapeQuery<'f>>,
        mut visit: impl FnMut(usize, &RayHit),
    ) {
        self.with_query(filter.into(), |world, filter| {
            world.cast_rays_closest(self.simd, rays, filter, |index, hit| {
                visit(index, &self.ray_hit(hit));
            });
        });
    }

    pub fn closest_point<'f>(
        &self,
        point: Vec3,
        max_distance: f32,
        filter: impl Into<ShapeQuery<'f>>,
    ) -> Option<ClosestPoint> {
        self.with_query(filter.into(), |world, filter| {
            world
                .closest_point(point, max_distance, filter)
                .map(|hit: PointHit| ClosestPoint {
                    shape: self.shape_id(hit.collider),
                    body: self.body_id(self.colliders[hit.collider as usize].owner),
                    point: hit.point,
                    normal: hit.normal,
                    distance: hit.distance,
                    triangle: hit.triangle,
                    child: hit.child,
                    material: hit.material,
                })
        })
    }

    pub fn overlap_bounds<'f>(
        &self,
        bounds: Aabb,
        filter: impl Into<ShapeQuery<'f>>,
        mut visit: impl FnMut(ShapeId) -> bool,
    ) {
        self.with_query(filter.into(), |world, filter| {
            world.overlap_bounds(&bounds, filter, |collider| visit(self.shape_id(collider)));
        });
    }

    pub fn overlap_shape<'f>(
        &self,
        proxy: &ShapeProxy<'_>,
        transform: Transform,
        filter: impl Into<ShapeQuery<'f>>,
        mut visit: impl FnMut(ShapeId) -> bool,
    ) {
        self.with_query(filter.into(), |world, filter| {
            world.overlap_shape(proxy, transform, filter, |collider| {
                visit(self.shape_id(collider))
            });
        });
    }

    pub fn cast_shape<'f>(
        &self,
        proxy: &ShapeProxy<'_>,
        transform: Transform,
        translation: Vec3,
        filter: impl Into<ShapeQuery<'f>>,
    ) -> Option<RayHit> {
        self.with_query(filter.into(), |world, filter| {
            world
                .cast_shape(proxy, transform, translation, filter)
                .map(|hit| self.ray_hit(&hit))
        })
    }
}
