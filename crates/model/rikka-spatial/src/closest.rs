use glam::{DVec3, Vec3};

use crate::ray::Ray;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentPoint {
    pub point: Vec3,
    pub parameter: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrianglePoint {
    pub point: Vec3,
    pub barycentric: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentPair {
    pub first: SegmentPoint,
    pub second: SegmentPoint,
    pub distance_squared: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaySegmentPair {
    pub ray_distance: f32,
    pub ray_point: Vec3,
    pub segment: SegmentPoint,
    pub distance_squared: f32,
}

pub fn closest_point_on_segment(point: Vec3, start: Vec3, end: Vec3) -> SegmentPoint {
    let direction = end - start;
    let length_squared = direction.length_squared();
    let parameter = match length_squared > 0.0 {
        true => direction.dot(point - start) / length_squared,
        false => 0.0,
    };
    if parameter <= 0.0 {
        return SegmentPoint {
            point: start,
            parameter: 0.0,
        };
    }
    if parameter >= 1.0 {
        return SegmentPoint {
            point: end,
            parameter: 1.0,
        };
    }
    SegmentPoint {
        point: start + direction * parameter,
        parameter,
    }
}

pub fn closest_point_on_triangle(point: Vec3, triangle: [Vec3; 3]) -> TrianglePoint {
    let [a, b, c] = triangle;
    let ab = b - a;
    let ac = c - a;
    let sine_squared_limit = f32::EPSILON * f32::EPSILON;
    if ab.cross(ac).length_squared()
        <= sine_squared_limit * ab.length_squared() * ac.length_squared()
    {
        return closest_on_edges(point, triangle);
    }
    let ap = point - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return TrianglePoint {
            point: a,
            barycentric: Vec3::X,
        };
    }
    let bp = point - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return TrianglePoint {
            point: b,
            barycentric: Vec3::Y,
        };
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return TrianglePoint {
            point: a + ab * v,
            barycentric: Vec3::new(1.0 - v, v, 0.0),
        };
    }
    let cp = point - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return TrianglePoint {
            point: c,
            barycentric: Vec3::Z,
        };
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return TrianglePoint {
            point: a + ac * w,
            barycentric: Vec3::new(1.0 - w, 0.0, w),
        };
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return TrianglePoint {
            point: b + (c - b) * w,
            barycentric: Vec3::new(0.0, 1.0 - w, w),
        };
    }
    let total = va + vb + vc;
    if total > 0.0 && total.is_finite() {
        let v = vb / total;
        let w = vc / total;
        return TrianglePoint {
            point: a + ab * v + ac * w,
            barycentric: Vec3::new(1.0 - v - w, v, w),
        };
    }
    closest_on_edges(point, triangle)
}

fn closest_on_edges(point: Vec3, triangle: [Vec3; 3]) -> TrianglePoint {
    let [a, b, c] = triangle;
    let candidates = [
        (closest_point_on_segment(point, a, b), 0usize, 1usize),
        (closest_point_on_segment(point, b, c), 1, 2),
        (closest_point_on_segment(point, c, a), 2, 0),
    ];
    let (closest, start, end) = candidates
        .into_iter()
        .min_by(|left, right| {
            left.0
                .point
                .distance_squared(point)
                .total_cmp(&right.0.point.distance_squared(point))
        })
        .expect("triangle has three edges");
    let mut barycentric = [0.0; 3];
    barycentric[start] = 1.0 - closest.parameter;
    barycentric[end] += closest.parameter;
    TrianglePoint {
        point: closest.point,
        barycentric: Vec3::from_array(barycentric),
    }
}

pub fn closest_points_segments(first: [Vec3; 2], second: [Vec3; 2]) -> SegmentPair {
    let solution = solve_lines(
        first[0].as_dvec3(),
        first[1].as_dvec3() - first[0].as_dvec3(),
        1.0,
        second,
    );
    let first_point = SegmentPoint {
        point: lerp(first, solution.first),
        parameter: solution.first as f32,
    };
    let second_point = SegmentPoint {
        point: lerp(second, solution.second),
        parameter: solution.second as f32,
    };
    SegmentPair {
        first: first_point,
        second: second_point,
        distance_squared: solution.distance_squared as f32,
    }
}

pub fn closest_points_ray_segment(
    ray: &Ray,
    max_distance: f32,
    segment: [Vec3; 2],
) -> RaySegmentPair {
    let solution = solve_lines(
        ray.origin().as_dvec3(),
        ray.direction().as_dvec3(),
        f64::from(max_distance),
        segment,
    );
    let ray_distance = solution.first as f32;
    RaySegmentPair {
        ray_distance,
        ray_point: (ray.origin().as_dvec3() + ray.direction().as_dvec3() * solution.first)
            .as_vec3(),
        segment: SegmentPoint {
            point: lerp(segment, solution.second),
            parameter: solution.second as f32,
        },
        distance_squared: solution.distance_squared as f32,
    }
}

fn lerp(segment: [Vec3; 2], parameter: f64) -> Vec3 {
    let [start, end] = segment.map(Vec3::as_dvec3);
    (start + (end - start) * parameter).as_vec3()
}

struct LineSolution {
    first: f64,
    second: f64,
    distance_squared: f64,
}

fn solve_lines(origin: DVec3, direction: DVec3, limit: f64, segment: [Vec3; 2]) -> LineSolution {
    let start = segment[0].as_dvec3();
    let edge = segment[1].as_dvec3() - start;
    let offset = origin - start;
    let a = direction.length_squared();
    let e = edge.length_squared();
    let f = edge.dot(offset);
    let clamp_first = |value: f64| match value.is_nan() {
        true => 0.0,
        false => value.clamp(0.0, limit),
    };
    let (first, second) = if a == 0.0 && e == 0.0 {
        (0.0, 0.0)
    } else if a == 0.0 {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = direction.dot(offset);
        if e == 0.0 {
            (clamp_first(-c / a), 0.0)
        } else {
            let b = direction.dot(edge);
            let denominator = a * e - b * b;
            let mut first = match denominator > 0.0 {
                true => clamp_first((b * f - c * e) / denominator),
                false => 0.0,
            };
            let mut second = (b * first + f) / e;
            if second < 0.0 {
                second = 0.0;
                first = clamp_first(-c / a);
            } else if second > 1.0 {
                second = 1.0;
                first = clamp_first((b - c) / a);
            }
            (first, second)
        }
    };
    let first_point = origin + direction * first;
    let second_point = start + edge * second;
    LineSolution {
        first,
        second,
        distance_squared: first_point.distance_squared(second_point),
    }
}
