use rikka_physics_math::{Aabb, Transform, Vec3};

use super::data::Compound;
use crate::triangulated::sweep::proxy_bounds;
use crate::{
    CastHit, MAX_PROXY_POINTS, MassData, RayInput, Shape, ShapeCastInput, ShapeGeometry, ShapeProxy,
};

impl Compound {
    pub fn query_triangles(
        &self,
        bounds: Aabb,
        mut visit: impl FnMut(Vec3, Vec3, Vec3, u32, u32) -> bool,
    ) -> bool {
        self.query_children(bounds, |index| {
            let child = self.child(index);
            let Shape::Mesh(mesh) = child.shape else {
                return true;
            };
            let local = bounds.transformed(child.transform.inverse());
            let transform = child.transform;
            mesh.query_triangles(local, |a, b, c, triangle| {
                visit(
                    transform.apply(a),
                    transform.apply(b),
                    transform.apply(c),
                    index,
                    triangle,
                )
            })
        })
    }

    #[inline]
    fn hit_material(child_material: u32, shape: &Shape, hit: &CastHit) -> u32 {
        match shape {
            Shape::Mesh(_) => child_material + hit.material,
            _ => child_material,
        }
    }
}

#[inline]
fn to_parent(hit: CastHit, transform: Transform, child: u32, material: u32) -> CastHit {
    CastHit {
        point: transform.apply(hit.point),
        normal: transform.rotate(hit.normal),
        child,
        material,
        ..hit
    }
}

impl ShapeGeometry for Compound {
    fn aabb(&self, transform: Transform) -> Aabb {
        self.bounds().transformed(transform)
    }

    fn mass(&self, density: f32) -> MassData {
        MassData::combine(self.children().map(|child| {
            let mut mass = child.shape.mass(density).rotated(child.transform.q);
            mass.center += child.transform.p;
            mass
        }))
    }

    fn ray_cast(&self, ray: &RayInput) -> Option<CastHit> {
        let order = self.order();
        let mut best: Option<CastHit> = None;
        self.tree().ray_cast(
            ray.origin,
            ray.translation,
            ray.max_fraction,
            |slot, limit| {
                let index = order[slot as usize];
                let child = self.child(index);
                let local = RayInput {
                    origin: child.transform.apply_inverse(ray.origin),
                    translation: child.transform.rotate_inverse(ray.translation),
                    max_fraction: limit,
                };
                let Some(hit) = child.shape.ray_cast(&local) else {
                    return limit;
                };
                if best.is_some_and(|best| hit.fraction >= best.fraction) {
                    return limit;
                }
                let material = Self::hit_material(child.material, child.shape, &hit);
                best = Some(to_parent(hit, child.transform, index, material));
                hit.fraction
            },
        );
        best
    }

    fn shape_cast(&self, cast: &ShapeCastInput<'_>) -> Option<CastHit> {
        let order = self.order();
        let bounds = proxy_bounds(&cast.proxy, |point| point);
        let mut points = [Vec3::ZERO; MAX_PROXY_POINTS];
        let mut best: Option<CastHit> = None;
        self.tree().box_cast(
            bounds,
            cast.translation,
            cast.max_fraction,
            |slot, limit| {
                let index = order[slot as usize];
                let child = self.child(index);
                let count = cast.proxy.points.len();
                for (target, &point) in points.iter_mut().zip(cast.proxy.points) {
                    *target = child.transform.apply_inverse(point);
                }
                let local = ShapeCastInput {
                    proxy: ShapeProxy::new(&points[..count], cast.proxy.radius),
                    translation: child.transform.rotate_inverse(cast.translation),
                    max_fraction: limit,
                    can_encroach: cast.can_encroach,
                };
                let Some(hit) = child.shape.shape_cast(&local) else {
                    return limit;
                };
                if best.is_some_and(|best| hit.fraction >= best.fraction) {
                    return limit;
                }
                let material = Self::hit_material(child.material, child.shape, &hit);
                best = Some(to_parent(hit, child.transform, index, material));
                hit.fraction
            },
        );
        best
    }

    fn overlaps(&self, transform: Transform, proxy: &ShapeProxy<'_>) -> bool {
        let bounds = proxy_bounds(proxy, |point| transform.apply_inverse(point));
        let mut overlapping = false;
        self.query_children(bounds, |index| {
            let child = self.child(index);
            overlapping = child.shape.overlaps(transform * child.transform, proxy);
            !overlapping
        });
        overlapping
    }
}
