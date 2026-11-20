mod closest;
mod mover;
mod packet;

pub use closest::PointHit;
pub use packet::Ray;

use rikka_physics_geometry::{CastHit, RayInput, ShapeCastInput, ShapeGeometry, ShapeProxy};
use rikka_physics_math::{Aabb, Transform, Vec3};

use crate::{BodyFrame, Broadphase, Collider, Material, QueryFilter};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryHit {
    pub collider: u32,
    pub point: Vec3,
    pub normal: Vec3,
    pub fraction: f32,
    pub triangle: u32,
    pub child: u32,
    pub material: Material,
}

#[derive(Clone, Copy)]
pub struct QueryWorld<'a> {
    pub broadphase: &'a Broadphase,
    pub colliders: &'a [Collider],
    pub frames: &'a [BodyFrame],
    pub custom: Option<&'a dyn Fn(u32) -> bool>,
}

impl std::fmt::Debug for QueryWorld<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryWorld")
            .field("colliders", &self.colliders.len())
            .field("custom", &self.custom.is_some())
            .finish()
    }
}

impl<'a> QueryWorld<'a> {
    fn frame(&self, collider: &Collider) -> Transform {
        self.frames[collider.owner as usize].transform
    }

    fn accepts(&self, index: u32, collider: &Collider, filter: &QueryFilter) -> bool {
        !collider.sensor
            && collider.filter.accepts(filter)
            && self.custom.is_none_or(|custom| custom(index))
    }

    fn hit(&self, index: u32, collider: &Collider, frame: Transform, local: CastHit) -> QueryHit {
        QueryHit {
            collider: index,
            point: frame.apply(local.point),
            normal: frame.rotate(local.normal),
            fraction: local.fraction,
            triangle: local.triangle,
            child: local.child,
            material: *collider.material(local.material),
        }
    }

    fn ray_shape(
        &self,
        index: u32,
        collider: &Collider,
        origin: Vec3,
        translation: Vec3,
        best: f32,
    ) -> Option<QueryHit> {
        let frame = self.frame(collider);
        let ray = RayInput {
            origin: frame.apply_inverse(origin),
            translation: frame.rotate_inverse(translation),
            max_fraction: best,
        };
        match collider.shape.ray_cast(&ray) {
            Some(local) if local.fraction <= best => Some(self.hit(index, collider, frame, local)),
            _ => None,
        }
    }

    pub fn cast_ray(
        &self,
        origin: Vec3,
        translation: Vec3,
        filter: &QueryFilter,
        mut visit: impl FnMut(&QueryHit) -> f32,
    ) {
        self.broadphase
            .ray_cast(origin, translation, 1.0, |index, best| {
                let collider = &self.colliders[index as usize];
                if !self.accepts(index, collider, filter) {
                    return best;
                }
                match self.ray_shape(index, collider, origin, translation, best) {
                    Some(hit) => visit(&hit),
                    None => best,
                }
            });
    }

    pub fn cast_ray_closest(
        &self,
        origin: Vec3,
        translation: Vec3,
        filter: &QueryFilter,
    ) -> Option<QueryHit> {
        let mut closest = None;
        self.cast_ray(origin, translation, filter, |hit| {
            closest = Some(*hit);
            hit.fraction
        });
        closest
    }

    pub fn overlap_bounds(
        &self,
        bounds: &Aabb,
        filter: &QueryFilter,
        mut visit: impl FnMut(u32) -> bool,
    ) {
        self.broadphase.query(bounds, |index| {
            let collider = &self.colliders[index as usize];
            if !self.accepts(index, collider, filter) {
                return true;
            }
            let actual = collider.shape.aabb(self.frame(collider));
            !actual.overlaps(bounds) || visit(index)
        });
    }

    pub fn overlap_shape(
        &self,
        proxy: &ShapeProxy<'_>,
        transform: Transform,
        filter: &QueryFilter,
        mut visit: impl FnMut(u32) -> bool,
    ) {
        let bounds = Aabb::from_points(proxy.points.iter().map(|&p| transform.apply(p)))
            .inflated(proxy.radius);
        self.broadphase.query(&bounds, |index| {
            let collider = &self.colliders[index as usize];
            if !self.accepts(index, collider, filter) {
                return true;
            }
            let frame = self.frame(collider);
            let relative = frame.inverse_mul(&transform);
            !collider.shape.overlaps(relative, proxy) || visit(index)
        });
    }

    pub fn cast_shape(
        &self,
        proxy: &ShapeProxy<'_>,
        transform: Transform,
        translation: Vec3,
        filter: &QueryFilter,
    ) -> Option<QueryHit> {
        let mut best: Option<QueryHit> = None;
        self.sweep_shape(proxy, transform, translation, filter, false, |hit| {
            if best.is_none_or(|current| hit.fraction < current.fraction) {
                best = Some(*hit);
            }
            best.map_or(1.0, |hit| hit.fraction)
        });
        best
    }

    fn sweep_shape(
        &self,
        proxy: &ShapeProxy<'_>,
        transform: Transform,
        translation: Vec3,
        filter: &QueryFilter,
        can_encroach: bool,
        mut visit: impl FnMut(&QueryHit) -> f32,
    ) {
        let start = Aabb::from_points(proxy.points.iter().map(|&p| transform.apply(p)))
            .inflated(proxy.radius);
        let swept = start.union(&start.translated(translation));
        let mut points = [Vec3::ZERO; rikka_physics_geometry::MAX_PROXY_POINTS];
        let mut best = 1.0f32;
        self.broadphase.query(&swept, |index| {
            let collider = &self.colliders[index as usize];
            if !self.accepts(index, collider, filter) {
                return true;
            }
            let frame = self.frame(collider);
            let relative = frame.inverse_mul(&transform);
            for (target, &source) in points.iter_mut().zip(proxy.points) {
                *target = relative.apply(source);
            }
            let local = ShapeCastInput {
                proxy: ShapeProxy::new(&points[..proxy.points.len()], proxy.radius),
                translation: frame.rotate_inverse(translation),
                max_fraction: best,
                can_encroach,
            };
            if let Some(hit) = collider.shape.shape_cast(&local) {
                best = visit(&self.hit(index, collider, frame, hit)).min(best);
            }
            true
        });
    }
}
