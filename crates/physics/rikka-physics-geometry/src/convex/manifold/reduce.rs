use rikka_physics_math::{Vec3, perpendicular};

use super::clip::ClipVertex;
use super::round::point;
use crate::convex::extrema::larger;
use crate::{MAX_MANIFOLD_POINTS, Manifold, ManifoldPoint};

const BIAS: f32 = 0.95;

pub(crate) trait Candidate: Copy {
    fn position(&self) -> Vec3;
    fn separation(&self) -> f32;
    fn manifold_point(&self) -> ManifoldPoint;
}

impl Candidate for ManifoldPoint {
    #[inline(always)]
    fn position(&self) -> Vec3 {
        self.point
    }

    #[inline(always)]
    fn separation(&self) -> f32 {
        self.separation
    }

    #[inline(always)]
    fn manifold_point(&self) -> ManifoldPoint {
        *self
    }
}

impl Candidate for ClipVertex {
    #[inline(always)]
    fn position(&self) -> Vec3 {
        self.point()
    }

    #[inline(always)]
    fn separation(&self) -> f32 {
        self.separation
    }

    #[inline(always)]
    fn manifold_point(&self) -> ManifoldPoint {
        point(self.point(), self.separation, self.pair)
    }
}

fn take<C: Candidate>(points: &mut [C], count: &mut usize, index: usize) -> C {
    let point = points[index];
    points[index] = points[*count - 1];
    *count -= 1;
    point
}

pub(crate) fn reduce<C: Candidate>(manifold: &mut Manifold, points: &mut [C], speculative: f32) {
    let mut count = points.len();
    manifold.count = 0;
    if count <= MAX_MANIFOLD_POINTS {
        for point in points.iter() {
            manifold.push(point.manifold_point());
        }
        return;
    }

    let normal = manifold.normal;
    let tolerance = speculative * speculative;
    let search = perpendicular(normal);

    let mut best = None;
    let mut best_score = -f32::MAX;
    for (index, point) in points[..count].iter().enumerate() {
        if point.separation() > speculative {
            continue;
        }
        let score = -point.separation() + search.dot(point.position());
        if BIAS * score > best_score {
            best = Some(index);
            best_score = score;
        }
    }
    let Some(first) = best else {
        return;
    };
    let a = take(points, &mut count, first);
    manifold.push(a.manifold_point());
    let a = a.position();

    let mut best = None;
    let mut best_score = 0.0;
    for (index, point) in points[..count].iter().enumerate() {
        let d = point.position() - a;
        let planar = d - d.dot(normal) * normal;
        let depth = larger(-point.separation(), 0.0);
        let score = planar.length_squared() + 4.0 * depth * depth;
        if BIAS * score > best_score {
            best_score = score;
            best = Some(index);
        }
    }
    let Some(second) = best.filter(|_| best_score >= tolerance) else {
        return;
    };
    let b = take(points, &mut count, second);
    manifold.push(b.manifold_point());
    let b = b.position();

    let ba = b - a;
    let mut best = None;
    let mut best_score = tolerance;
    let mut best_signed_area = 0.0;
    for (index, point) in points[..count].iter().enumerate() {
        let signed_area = normal.dot(ba.cross(point.position() - a));
        let score = signed_area.abs();
        if BIAS * score >= best_score {
            best_score = score;
            best = Some(index);
            best_signed_area = signed_area;
        }
    }
    let Some(third) = best else {
        return;
    };
    let c = take(points, &mut count, third);
    manifold.push(c.manifold_point());
    let c = c.position();

    let sign = if best_signed_area < 0.0 { -1.0 } else { 1.0 };
    let mut best = None;
    let mut best_score = tolerance;
    for (index, point) in points[..count].iter().enumerate() {
        let p = point.position();
        let u1 = sign * normal.dot((p - a).cross(ba));
        let u2 = sign * normal.dot((p - b).cross(c - b));
        let u3 = sign * normal.dot((p - c).cross(a - c));
        let score = larger(larger(u1, u2), u3);
        if BIAS * score > best_score {
            best_score = score;
            best = Some(index);
        }
    }
    if let Some(fourth) = best {
        manifold.push(points[fourth].manifold_point());
    }
}
