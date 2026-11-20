use rikka_physics_collision::Ray;
use rikka_physics_geometry::ShapeProxy;
use rikka_physics_math::{Aabb, Transform, Vec3};
use rikka_physics_rigid::{
    ClosestPoint, RayHit, SensorBeginEvent, SensorEndEvent, ShapeId, ShapeQuery,
};

use super::World;
use crate::Result;

impl World {
    pub fn cast_ray_closest<'f>(
        &self,
        origin: Vec3,
        translation: Vec3,
        filter: impl Into<ShapeQuery<'f>>,
    ) -> Option<RayHit> {
        self.rigid.cast_ray_closest(origin, translation, filter)
    }

    pub fn cast_ray<'f>(
        &self,
        origin: Vec3,
        translation: Vec3,
        filter: impl Into<ShapeQuery<'f>>,
        visit: impl FnMut(&RayHit) -> f32,
    ) {
        self.rigid.cast_ray(origin, translation, filter, visit);
    }

    pub fn cast_rays_closest<'f>(
        &self,
        rays: &[Ray],
        filter: impl Into<ShapeQuery<'f>>,
        visit: impl FnMut(usize, &RayHit),
    ) {
        self.rigid.cast_rays_closest(rays, filter, visit);
    }

    pub fn closest_point<'f>(
        &self,
        point: Vec3,
        max_distance: f32,
        filter: impl Into<ShapeQuery<'f>>,
    ) -> Option<ClosestPoint> {
        self.rigid.closest_point(point, max_distance, filter)
    }

    pub fn overlap_bounds<'f>(
        &self,
        bounds: Aabb,
        filter: impl Into<ShapeQuery<'f>>,
        visit: impl FnMut(ShapeId) -> bool,
    ) {
        self.rigid.overlap_bounds(bounds, filter, visit);
    }

    pub fn overlap_shape<'f>(
        &self,
        proxy: &ShapeProxy<'_>,
        transform: Transform,
        filter: impl Into<ShapeQuery<'f>>,
        visit: impl FnMut(ShapeId) -> bool,
    ) {
        self.rigid.overlap_shape(proxy, transform, filter, visit);
    }

    pub fn cast_shape<'f>(
        &self,
        proxy: &ShapeProxy<'_>,
        transform: Transform,
        translation: Vec3,
        filter: impl Into<ShapeQuery<'f>>,
    ) -> Option<RayHit> {
        self.rigid.cast_shape(proxy, transform, translation, filter)
    }

    pub fn sensor_begin_events(&self) -> &[SensorBeginEvent] {
        &self.events.sensor_begins
    }

    pub fn sensor_end_events(&self) -> &[SensorEndEvent] {
        &self.events.sensor_ends
    }

    pub fn sensor_overlaps(&self, sensor: ShapeId) -> Result<impl Iterator<Item = ShapeId> + '_> {
        Ok(self.rigid.sensor_overlaps(sensor)?)
    }
}
