use rikka_physics_math::{Aabb, Transform, Vec3};

use crate::triangulated::triangle::{Triangle, ray_triangle, signed_volume};
use crate::{
    CastHit, DistanceInput, RayInput, ShapeCastInput, ShapeCastPairInput, ShapeProxy, SimplexCache,
    Tolerance, shape_cast_pair, shape_distance,
};

pub(crate) trait TriangleSource {
    fn triangles(&self) -> impl Fn(u32) -> Triangle + '_;

    fn sweep_candidates(
        &self,
        origin: Vec3,
        translation: Vec3,
        extent: Vec3,
        max_fraction: f32,
        visit: impl FnMut(Triangle, f32) -> f32,
    ) -> f32;

    fn overlap_candidates(&self, bounds: Aabb, visit: impl FnMut(Triangle) -> bool) -> bool;

    fn overlapping(&self, bounds: Aabb, visit: impl FnMut(u32) -> bool) -> bool;
}

#[inline]
fn hit(point: Vec3, normal: Vec3, fraction: f32, triangle: &Triangle) -> CastHit {
    CastHit {
        triangle: triangle.index,
        material: u32::from(triangle.material),
        ..CastHit::new(point, normal, fraction)
    }
}

pub(crate) fn ray_cast_triangles(source: &impl TriangleSource, ray: &RayInput) -> Option<CastHit> {
    let mut best: Option<CastHit> = None;
    source.sweep_candidates(
        ray.origin,
        ray.translation,
        Vec3::ZERO,
        ray.max_fraction,
        |triangle, limit| {
            let Some(fraction) =
                ray_triangle(ray.origin, ray.translation, triangle.vertices, limit)
            else {
                return limit;
            };
            if best.is_some_and(|best| fraction >= best.fraction) {
                return limit;
            }
            let point = ray.origin + fraction * ray.translation;
            best = Some(hit(point, triangle.normal(), fraction, &triangle));
            fraction
        },
    );
    best
}

pub(crate) fn proxy_bounds(proxy: &ShapeProxy<'_>, map: impl Fn(Vec3) -> Vec3) -> Aabb {
    Aabb::from_points(proxy.points.iter().map(|&point| map(point))).inflated(proxy.radius)
}

pub(crate) fn shape_cast_triangles(
    source: &impl TriangleSource,
    cast: &ShapeCastInput<'_>,
) -> Option<CastHit> {
    let bounds = proxy_bounds(&cast.proxy, |point| point);
    let center = bounds.center();
    let mut best: Option<CastHit> = None;
    source.sweep_candidates(
        center,
        cast.translation,
        bounds.extent(),
        cast.max_fraction,
        |triangle, limit| {
            let [a, b, c] = triangle.vertices;
            if signed_volume(a, b, c, center) < 0.0 {
                return limit;
            }
            let shifted = [Vec3::ZERO, b - a, c - a];
            let proxy = ShapeProxy::new(&shifted, 0.0);
            let mut input = ShapeCastPairInput::new(
                &proxy,
                &cast.proxy,
                Transform::from_translation(-a),
                cast.translation,
            );
            input.max_fraction = limit;
            input.can_encroach = cast.can_encroach;
            let Some(output) = shape_cast_pair(&input) else {
                return limit;
            };
            if best.is_some_and(|best| output.fraction >= best.fraction) {
                return limit;
            }
            best = Some(hit(
                output.point + a,
                output.normal,
                output.fraction,
                &triangle,
            ));
            output.fraction
        },
    );
    best
}

pub(crate) fn overlap_triangles(
    source: &impl TriangleSource,
    transform: Transform,
    proxy: &ShapeProxy<'_>,
) -> bool {
    let bounds = proxy_bounds(proxy, |point| transform.apply_inverse(point));
    let inverse = transform.inverse();
    let tolerance = 0.1 * Tolerance::default().linear_slop();
    let mut overlapping = false;
    source.overlap_candidates(bounds, |triangle| {
        let triangle_proxy = ShapeProxy::new(&triangle.vertices, 0.0);
        let input = DistanceInput::new(&triangle_proxy, proxy, inverse).with_radii();
        let mut cache = SimplexCache::EMPTY;
        let output = shape_distance(&input, &mut cache);
        overlapping = output.distance < tolerance;
        !overlapping
    });
    overlapping
}
