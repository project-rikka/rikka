use rikka_physics_math::{Mat3, Transform, Vec3};

use super::cache::{NO_HEADING, SimplexCache, heading};
use super::simplex::{MAX_SIMPLEX_VERTICES, Simplex, SimplexVertex};
use crate::ShapeProxy;
use crate::convex::support::{Support, SupportPoint};

const MAX_ITERATIONS: u32 = 32;
const CONVERGENCE: f32 = 1.0e-5;

#[derive(Debug)]
pub struct DistanceInput<'a, A: ?Sized = ShapeProxy<'a>, B: ?Sized = ShapeProxy<'a>> {
    pub a: &'a A,
    pub b: &'a B,
    pub transform: Transform,
    pub use_radii: bool,
}

impl<'a, A: Support + ?Sized, B: Support + ?Sized> DistanceInput<'a, A, B> {
    #[inline]
    pub fn new(a: &'a A, b: &'a B, transform: Transform) -> Self {
        Self {
            a,
            b,
            transform,
            use_radii: false,
        }
    }

    #[inline]
    #[must_use]
    pub fn with_radii(mut self) -> Self {
        self.use_radii = true;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DistanceOutput {
    pub point_a: Vec3,
    pub point_b: Vec3,
    pub normal: Vec3,
    pub distance: f32,
    pub iterations: u32,
}

struct Frame {
    rotation: Mat3,
    inverse: Mat3,
    translation: Vec3,
}

impl Frame {
    fn new(transform: Transform) -> Self {
        let rotation = Mat3::from_quat(transform.q);
        Self {
            rotation,
            inverse: rotation.transpose(),
            translation: transform.p,
        }
    }

    #[inline]
    fn vertex(&self, a: SupportPoint, b: SupportPoint, headings: [u32; 2]) -> SimplexVertex {
        SimplexVertex::new(a, b, self.rotation * b.point + self.translation, headings)
    }
}

#[inline(always)]
fn warm_simplex<A: Support + ?Sized, B: Support + ?Sized>(
    cache: &SimplexCache,
    frame: &Frame,
    a: &A,
    b: &B,
) -> Simplex {
    let mut simplex = Simplex::default();
    if cache.count == 0 {
        return simplex;
    }
    let Some(cached) = cache.vertices(a, b) else {
        return simplex;
    };
    simplex.count = cache.count as usize;
    for (vertex, cached) in simplex.vertices.iter_mut().zip(cached).take(simplex.count) {
        *vertex = frame.vertex(cached.a, cached.b, [cached.heading_a, cached.heading_b]);
    }
    let old = cache.metric;
    let new = simplex.metric();
    if 2.0 * old < new || new < 0.5 * old || new < f32::EPSILON {
        simplex.count = 0;
    }
    simplex
}

pub fn shape_distance<A: Support + ?Sized, B: Support + ?Sized>(
    input: &DistanceInput<'_, A, B>,
    cache: &mut SimplexCache,
) -> DistanceOutput {
    gjk(input, cache).0
}

pub(crate) fn gjk<A: Support + ?Sized, B: Support + ?Sized>(
    input: &DistanceInput<'_, A, B>,
    cache: &mut SimplexCache,
) -> (DistanceOutput, Simplex) {
    let frame = Frame::new(input.transform);
    let a = input.a;
    let b = input.b;

    let mut simplex = warm_simplex(cache, &frame, a, b);
    if simplex.count == 0 {
        simplex.count = 1;
        simplex.vertices[0] = frame.vertex(a.anchor(), b.anchor(), [NO_HEADING; 2]);
    }

    let mut backup = simplex;
    let mut distance_squared = f32::MAX;
    let mut normal = Vec3::ZERO;
    let mut iteration = 0;

    while iteration < MAX_ITERATIONS {
        if !simplex.solve() {
            simplex = backup;
            break;
        }

        if simplex.count == MAX_SIMPLEX_VERTICES {
            return (overlap(&simplex, iteration), simplex);
        }

        let old_distance_squared = distance_squared;
        let closest = simplex.closest_point();
        distance_squared = closest.length_squared();
        if distance_squared >= old_distance_squared {
            simplex = backup;
            break;
        }

        let direction = simplex.search_direction();
        if direction.length_squared() < 1000.0 * f32::MIN_POSITIVE {
            return (overlap(&simplex, iteration), simplex);
        }
        normal = -direction;

        let direction_a = -direction;
        let direction_b = frame.inverse * direction;
        let support_a = a.support(direction_a);
        let support_b = b.support(direction_b);
        let mut vertex = frame.vertex(support_a, support_b, [NO_HEADING; 2]);

        let duplicate = simplex.vertices[..simplex.count].iter().any(|vertex| {
            vertex.support_a().same_feature(&support_a)
                && vertex.support_b().same_feature(&support_b)
        });
        if duplicate {
            break;
        }

        if distance_squared - closest.dot(vertex.w) <= CONVERGENCE * distance_squared {
            break;
        }

        backup = simplex;

        vertex.heading_a = heading(support_a, direction_a);
        vertex.heading_b = heading(support_b, direction_b);
        simplex.vertices[simplex.count] = vertex;
        simplex.count += 1;
        iteration += 1;
    }

    let (point_a, point_b) = simplex.witness_points();
    let mut output = DistanceOutput {
        point_a,
        point_b,
        normal: Vec3::ZERO,
        distance: 0.0,
        iterations: iteration,
    };

    let normal = normal.normalize_or_zero();
    if !is_normalized(normal) {
        return (output, simplex);
    }

    output.distance = point_a.distance(point_b);
    output.normal = normal;

    if input.use_radii {
        let radius_a = a.radius();
        let radius_b = b.radius();
        output.distance = (output.distance - radius_a - radius_b).max(0.0);
        output.point_a += radius_a * normal;
        output.point_b -= radius_b * normal;
    }

    cache.write(&simplex);
    (output, simplex)
}

fn overlap(simplex: &Simplex, iteration: u32) -> DistanceOutput {
    let (point_a, point_b) = simplex.witness_points();
    DistanceOutput {
        point_a,
        point_b,
        normal: Vec3::ZERO,
        distance: 0.0,
        iterations: iteration,
    }
}

#[inline]
pub(crate) fn is_normalized(v: Vec3) -> bool {
    (v.length_squared() - 1.0).abs() < 100.0 * f32::EPSILON
}
