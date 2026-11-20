use rikka_physics_math::{Vec2, Vec3, perpendicular};

use crate::ManifoldPoint;

#[derive(Clone, Copy, Debug)]
struct Projected {
    p: Vec2,
    separation: f32,
    index: usize,
}

#[inline]
fn better(
    score: f32,
    separation: f32,
    best_score: f32,
    best_separation: f32,
    score_tolerance: f32,
    separation_tolerance: f32,
) -> bool {
    if score > best_score + score_tolerance {
        return true;
    }
    if score < best_score - score_tolerance {
        return false;
    }
    separation < best_separation - separation_tolerance
}

#[inline]
fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

#[derive(Debug, Default)]
pub(crate) struct Culler {
    projected: Vec<Projected>,
    suffix: Vec<(Vec2, Vec2)>,
    scratch: Vec<ManifoldPoint>,
}

impl Culler {
    pub(crate) fn reduce(
        &mut self,
        points: &mut Vec<ManifoldPoint>,
        normal: Vec3,
        linear_slop: f32,
    ) {
        if points.len() <= 1 {
            return;
        }
        let u = perpendicular(normal);
        let v = normal.cross(u);
        let origin = points[0].point;
        self.projected.clear();
        self.projected
            .extend(points.iter().enumerate().map(|(index, point)| {
                let d = point.point - origin;
                Projected {
                    p: Vec2::new(d.dot(u), d.dot(v)),
                    separation: point.separation,
                    index,
                }
            }));
        suffix_bounds(&self.projected, &mut self.suffix);
        let chosen = cull(&mut self.projected, &self.suffix, linear_slop);
        self.scratch.clear();
        self.scratch
            .extend(chosen.iter().flatten().map(|&index| points[index]));
        std::mem::swap(points, &mut self.scratch);
    }
}

fn suffix_bounds(points: &[Projected], suffix: &mut Vec<(Vec2, Vec2)>) {
    suffix.clear();
    suffix.resize(points.len(), (Vec2::ZERO, Vec2::ZERO));
    let mut low = Vec2::splat(f32::MAX);
    let mut high = Vec2::splat(f32::MIN);
    for (point, bounds) in points.iter().zip(suffix.iter_mut()).rev() {
        low = low.min(point.p);
        high = high.max(point.p);
        *bounds = (low, high);
    }
}

#[inline]
fn farthest_squared(p: Vec2, (low, high): (Vec2, Vec2)) -> f32 {
    let x = (p.x - low.x).max(high.x - p.x);
    let y = (p.y - low.y).max(high.y - p.y);
    x * x + y * y
}

fn cull(
    points: &mut Vec<Projected>,
    suffix: &[(Vec2, Vec2)],
    linear_slop: f32,
) -> [Option<usize>; 4] {
    let tolerance = 0.25 * linear_slop;
    let tolerance_squared = tolerance * tolerance;
    let separation_tolerance = linear_slop;
    let mut chosen = [None; 4];

    let mut best_score = 0.0;
    let mut best_separation = f32::MAX;
    let mut best_pair = None;
    for i in 0..points.len().saturating_sub(1) {
        if farthest_squared(points[i].p, suffix[i + 1]) < best_score - tolerance_squared {
            continue;
        }
        for j in i + 1..points.len() {
            let score = points[i].p.distance_squared(points[j].p);
            let separation = points[i].separation + points[j].separation;
            if better(
                score,
                separation,
                best_score,
                best_separation,
                tolerance_squared,
                separation_tolerance,
            ) {
                best_pair = Some((i, j));
                best_score = score;
                best_separation = separation;
            }
        }
    }
    let Some((first, second)) = best_pair.filter(|_| best_score >= tolerance_squared) else {
        let deepest = points.iter().enumerate().fold(0, |best, (index, point)| {
            if point.separation < points[best].separation {
                index
            } else {
                best
            }
        });
        chosen[0] = Some(points[deepest].index);
        return chosen;
    };
    let a = points[first];
    let mut b = points[second];
    chosen[0] = Some(a.index);
    chosen[1] = Some(b.index);
    points.swap_remove(second);
    points.swap_remove(first);
    if points.is_empty() {
        return chosen;
    }

    let mut ba = b.p - a.p;
    best_score = 0.0;
    best_separation = f32::MAX;
    let mut best = None;
    let mut best_signed_area = 0.0;
    for (index, point) in points.iter().enumerate() {
        let signed_area = cross(ba, point.p - a.p);
        let score = signed_area.abs();
        if score <= tolerance_squared {
            continue;
        }
        if better(
            score,
            point.separation,
            best_score,
            best_separation,
            tolerance_squared,
            separation_tolerance,
        ) {
            best_signed_area = signed_area;
            best_score = score;
            best_separation = point.separation;
            best = Some(index);
        }
    }
    let Some(third) = best else {
        return chosen;
    };
    let mut c = points.swap_remove(third);
    chosen[2] = Some(c.index);
    if points.is_empty() {
        return chosen;
    }

    if best_signed_area < 0.0 {
        std::mem::swap(&mut b, &mut c);
        ba = b.p - a.p;
    }
    let cb = c.p - b.p;
    let ac = a.p - c.p;
    best_score = 0.0;
    best_separation = f32::MAX;
    let mut best = None;
    for (index, point) in points.iter().enumerate() {
        let u1 = cross(point.p - a.p, ba);
        let u2 = cross(point.p - b.p, cb);
        let u3 = cross(point.p - c.p, ac);
        let score = u1.max(u2).max(u3);
        if score <= tolerance_squared {
            continue;
        }
        if better(
            score,
            point.separation,
            best_score,
            best_separation,
            tolerance_squared,
            separation_tolerance,
        ) {
            best_score = score;
            best_separation = point.separation;
            best = Some(index);
        }
    }
    if let Some(fourth) = best {
        chosen[3] = Some(points[fourth].index);
    }
    chosen
}
