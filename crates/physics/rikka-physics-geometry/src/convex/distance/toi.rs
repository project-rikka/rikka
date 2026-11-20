use rikka_physics_math::{Quat, Transform, Vec3, nlerp};

use super::cache::SimplexCache;
use super::gjk::{DistanceInput, shape_distance};
use crate::convex::support::{Support, SupportPoint};
use crate::{ShapeProxy, Tolerance};

const MAX_ITERATIONS: u32 = 25;
const MAX_ROOT_ITERATIONS: u32 = 50;
const MAX_PUSH_BACK_ITERATIONS: u32 = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sweep {
    pub local_center: Vec3,
    pub c1: Vec3,
    pub c2: Vec3,
    pub q1: Quat,
    pub q2: Quat,
}

impl Sweep {
    #[inline]
    pub fn fixed(transform: Transform, local_center: Vec3) -> Self {
        let center = transform.apply(local_center);
        Self {
            local_center,
            c1: center,
            c2: center,
            q1: transform.q,
            q2: transform.q,
        }
    }

    #[inline]
    pub fn from_transforms(from: Transform, to: Transform, local_center: Vec3) -> Self {
        Self {
            local_center,
            c1: from.apply(local_center),
            c2: to.apply(local_center),
            q1: from.q,
            q2: to.q,
        }
    }

    #[inline]
    pub fn transform(&self, time: f32) -> Transform {
        let q = nlerp(self.q1, self.q2, time);
        Transform::new(self.c1.lerp(self.c2, time) - q * self.local_center, q)
    }

    #[inline]
    fn end(&self) -> Transform {
        Transform::new(self.c2 - self.q2 * self.local_center, self.q2)
    }
}

#[derive(Debug)]
pub struct ToiInput<'a, A: ?Sized = ShapeProxy<'a>, B: ?Sized = ShapeProxy<'a>> {
    pub a: &'a A,
    pub b: &'a B,
    pub sweep_a: Sweep,
    pub sweep_b: Sweep,
    pub max_fraction: f32,
    pub tolerance: Tolerance,
}

impl<'a, A: Support + ?Sized, B: Support + ?Sized> ToiInput<'a, A, B> {
    #[inline]
    pub fn new(a: &'a A, sweep_a: Sweep, b: &'a B, sweep_b: Sweep) -> Self {
        Self {
            a,
            b,
            sweep_a,
            sweep_b,
            max_fraction: 1.0,
            tolerance: Tolerance::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToiState {
    Failed,
    Overlapped,
    Hit,
    Separated,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToiOutput {
    pub state: ToiState,
    pub point: Vec3,
    pub normal: Vec3,
    pub fraction: f32,
    pub distance: f32,
}

#[derive(Clone, Copy, Debug)]
enum Separation {
    Points { axis: Vec3 },
    Edges { edge_a: Vec3, edge_b: Vec3 },
    FaceA { normal: Vec3, point: Vec3 },
    FaceB { normal: Vec3, point: Vec3 },
}

struct SeparationFunction<'s, A: ?Sized, B: ?Sized> {
    a: &'s A,
    b: &'s B,
    sweep_a: Sweep,
    sweep_b: Sweep,
    kind: Separation,
}

fn unique_count(points: &[SupportPoint]) -> usize {
    match points.len() {
        1 => 1,
        2 => {
            if points[0].same_feature(&points[1]) {
                1
            } else {
                2
            }
        }
        _ => {
            let ab = points[0].same_feature(&points[1]);
            let ac = points[0].same_feature(&points[2]);
            let bc = points[1].same_feature(&points[2]);
            if !ab && !ac && !bc {
                3
            } else if ab && ac && bc {
                1
            } else {
                2
            }
        }
    }
}

fn distinct_pair(points: &[SupportPoint]) -> (Vec3, Vec3) {
    if points[0].same_feature(&points[1]) {
        (points[0].point, points[2].point)
    } else {
        (points[0].point, points[1].point)
    }
}

fn fast_edges(xf_a: Transform, edge_a: Vec3, xf_b: Transform, edge_b: Vec3, axis: Vec3) -> bool {
    (xf_a.q * edge_a).cross(xf_b.q * edge_b).dot(axis) < 0.0
}

impl<'s, A: Support + ?Sized, B: Support + ?Sized> SeparationFunction<'s, A, B> {
    fn edge_or_points(
        &self,
        (a1, a2): (Vec3, Vec3),
        (b1, b2): (Vec3, Vec3),
        tolerance: f32,
        world_normal: Vec3,
        t1: f32,
    ) -> Separation {
        let xf_a = self.sweep_a.transform(t1);
        let xf_b = self.sweep_b.transform(t1);
        let local_a = (a2 - a1).normalize_or_zero();
        let mut local_b = (b2 - b1).normalize_or_zero();
        let mut axis = (xf_a.q * local_a).cross(xf_b.q * local_b);
        if axis.length_squared() < tolerance * tolerance {
            return Separation::Points { axis: world_normal };
        }
        let delta = xf_b.q * b1 - xf_a.q * a1 + (xf_b.p - xf_a.p);
        if delta.dot(axis) < 0.0 {
            axis = -axis;
            local_b = -local_b;
        }
        if fast_edges(
            self.sweep_a.end(),
            local_a,
            self.sweep_b.end(),
            local_b,
            axis,
        ) {
            Separation::Points {
                axis: axis.normalize(),
            }
        } else {
            Separation::Edges {
                edge_a: local_a,
                edge_b: local_b,
            }
        }
    }

    fn new(
        cache: &SimplexCache,
        a: &'s A,
        sweep_a: Sweep,
        b: &'s B,
        sweep_b: Sweep,
        world_normal: Vec3,
        t1: f32,
    ) -> Self {
        let mut function = Self {
            a,
            b,
            sweep_a,
            sweep_b,
            kind: Separation::Points { axis: world_normal },
        };
        let Some(cached) = cache.vertices(a, b) else {
            return function;
        };
        let count = cache.count as usize;
        let xf_a = sweep_a.transform(t1);
        let xf_b = sweep_b.transform(t1);
        let points_a = cached.map(|vertex| vertex.a);
        let points_b = cached.map(|vertex| vertex.b);
        let (points_a, points_b) = (&points_a[..count], &points_b[..count]);
        let unique_a = unique_count(points_a);
        let unique_b = unique_count(points_b);
        let delta_p = xf_b.p - xf_a.p;

        function.kind = match count {
            2 if unique_a == 2 && unique_b == 2 => function.edge_or_points(
                (points_a[0].point, points_a[1].point),
                (points_b[0].point, points_b[1].point),
                0.05,
                world_normal,
                t1,
            ),
            3 if unique_a == 3 => {
                let [a1, a2, a3] = [points_a[0].point, points_a[1].point, points_a[2].point];
                let mut normal = (a2 - a1).cross(a3 - a1).normalize();
                let point = (a1 + a2 + a3) / 3.0;
                let delta = xf_b.q * points_b[0].point - xf_a.q * point + delta_p;
                if delta.dot(xf_a.q * normal) < 0.0 {
                    normal = -normal;
                }
                Separation::FaceA { normal, point }
            }
            3 if unique_b == 3 => {
                let [b1, b2, b3] = [points_b[0].point, points_b[1].point, points_b[2].point];
                let mut normal = (b2 - b1).cross(b3 - b1).normalize();
                let point = (b1 + b2 + b3) / 3.0;
                let delta = xf_a.q * points_a[0].point - xf_b.q * point - delta_p;
                if delta.dot(xf_b.q * normal) < 0.0 {
                    normal = -normal;
                }
                Separation::FaceB { normal, point }
            }
            3 if unique_a == 2 && unique_b == 2 => function.edge_or_points(
                distinct_pair(points_a),
                distinct_pair(points_b),
                0.005,
                world_normal,
                t1,
            ),
            _ => Separation::Points { axis: world_normal },
        };
        function
    }

    fn find_min_separation(&self, t: f32) -> (f32, Vec3, Vec3) {
        let xf_a = self.sweep_a.transform(t);
        let xf_b = self.sweep_b.transform(t);
        match self.kind {
            Separation::Points { axis } => {
                let pa = self.a.support(xf_a.rotate_inverse(axis)).point;
                let pb = self.b.support(xf_b.rotate_inverse(-axis)).point;
                ((xf_b.apply(pb) - xf_a.apply(pa)).dot(axis), pa, pb)
            }
            Separation::Edges { edge_a, edge_b } => {
                let axis = (xf_a.q * edge_a).cross(xf_b.q * edge_b).normalize();
                let pa = self.a.support(xf_a.rotate_inverse(axis)).point;
                let pb = self.b.support(xf_b.rotate_inverse(-axis)).point;
                ((xf_b.apply(pb) - xf_a.apply(pa)).dot(axis), pa, pb)
            }
            Separation::FaceA { normal, point } => {
                let axis = xf_a.q * normal;
                let pb = self.b.support(xf_b.rotate_inverse(-axis)).point;
                ((xf_b.apply(pb) - xf_a.apply(point)).dot(axis), point, pb)
            }
            Separation::FaceB { normal, point } => {
                let axis = xf_b.q * normal;
                let pa = self.a.support(xf_a.rotate_inverse(-axis)).point;
                ((xf_a.apply(pa) - xf_b.apply(point)).dot(axis), pa, point)
            }
        }
    }

    fn evaluate(&self, pa: Vec3, pb: Vec3, t: f32) -> f32 {
        let xf_a = self.sweep_a.transform(t);
        let xf_b = self.sweep_b.transform(t);
        match self.kind {
            Separation::Points { axis } => (xf_b.apply(pb) - xf_a.apply(pa)).dot(axis),
            Separation::Edges { edge_a, edge_b } => {
                let axis = (xf_a.q * edge_a).cross(xf_b.q * edge_b).normalize();
                (xf_b.apply(pb) - xf_a.apply(pa)).dot(axis)
            }
            Separation::FaceA { normal, point } => {
                (xf_b.apply(pb) - xf_a.apply(point)).dot(xf_a.q * normal)
            }
            Separation::FaceB { normal, point } => {
                (xf_a.apply(pa) - xf_b.apply(point)).dot(xf_b.q * normal)
            }
        }
    }

    fn force_fixed_axis(&mut self, t: f32) {
        if let Separation::Edges { edge_a, edge_b } = self.kind {
            let xf_a = self.sweep_a.transform(t);
            let xf_b = self.sweep_b.transform(t);
            let axis = (xf_a.q * edge_a).cross(xf_b.q * edge_b).normalize();
            self.kind = Separation::Points { axis };
        }
    }
}

pub fn time_of_impact<A: Support + ?Sized, B: Support + ?Sized>(
    input: &ToiInput<'_, A, B>,
) -> ToiOutput {
    let origin = input.sweep_a.c1;
    let mut sweep_a = input.sweep_a;
    let mut sweep_b = input.sweep_b;
    sweep_a.c1 = Vec3::ZERO;
    sweep_a.c2 -= origin;
    sweep_b.c1 -= origin;
    sweep_b.c2 -= origin;

    let radius_a = input.a.radius();
    let radius_b = input.b.radius();
    let t_max = input.max_fraction;
    let linear_slop = input.tolerance.linear_slop();
    let target = linear_slop.max(radius_a + radius_b - linear_slop);
    let tolerance = 0.25 * linear_slop;

    let mut t1 = 0.0f32;
    let mut cache = SimplexCache::EMPTY;
    let mut iterations = 0;

    loop {
        let xf_a = sweep_a.transform(t1);
        let xf_b = sweep_b.transform(t1);
        let distance_input = DistanceInput::new(input.a, input.b, xf_a.inverse_mul(&xf_b));
        let output = shape_distance(&distance_input, &mut cache);
        iterations += 1;

        let world_normal = xf_a.q * output.normal;
        let hit_point = {
            let pa = xf_a.apply(output.point_a) + radius_a * world_normal;
            let pb = xf_a.apply(output.point_b) - radius_b * world_normal;
            pa.lerp(pb, 0.5) + origin
        };
        let finish = |state, fraction| ToiOutput {
            state,
            point: hit_point,
            normal: world_normal,
            fraction,
            distance: output.distance,
        };

        if output.distance <= 0.0 {
            return finish(ToiState::Overlapped, 0.0);
        }
        if output.distance <= target + tolerance {
            return finish(ToiState::Hit, t1);
        }
        if iterations == MAX_ITERATIONS {
            return finish(ToiState::Failed, t1);
        }

        let mut function =
            SeparationFunction::new(&cache, input.a, sweep_a, input.b, sweep_b, world_normal, t1);

        let mut t2 = t_max;
        let mut push_back = 0;
        loop {
            let (mut s2, pa, pb) = function.find_min_separation(t2);

            if s2 - target > tolerance {
                return finish(ToiState::Separated, t_max);
            }
            if s2 >= target - tolerance {
                t1 = t2;
                break;
            }

            let mut s1 = function.evaluate(pa, pb, t1);
            if s1 < target - tolerance {
                return finish(ToiState::Failed, t1);
            }
            if s1 <= target + tolerance {
                return finish(ToiState::Hit, t1);
            }

            let mut root_iterations = 0;
            let mut a1 = t1;
            let mut a2 = t2;
            loop {
                let t = if root_iterations & 1 == 1 {
                    a1 + (target - s1) * (a2 - a1) / (s2 - s1)
                } else {
                    0.5 * (a1 + a2)
                };
                root_iterations += 1;

                let s = function.evaluate(pa, pb, t);
                if (s - target).abs() <= tolerance {
                    t2 = t;
                    break;
                }
                if s > target {
                    a1 = t;
                    s1 = s;
                } else {
                    a2 = t;
                    s2 = s;
                }
                if root_iterations == MAX_ROOT_ITERATIONS {
                    break;
                }
            }

            if root_iterations == MAX_ROOT_ITERATIONS - 1
                && matches!(function.kind, Separation::Edges { .. })
            {
                t2 = t_max;
                function.force_fixed_axis(t1);
            }

            push_back += 1;
            if push_back == MAX_PUSH_BACK_ITERATIONS {
                break;
            }
        }
    }
}
