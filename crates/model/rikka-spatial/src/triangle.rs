use glam::{DVec2, DVec3, Vec2, Vec3};
use smallvec::SmallVec;

use crate::aabb::Aabb;
use crate::closest::closest_point_on_segment;
use crate::polygon::{SegmentIntersection2d, intersect_segments_2d};
use crate::predicates::{orient2d, orient3d};

#[derive(Debug, Clone, PartialEq)]
pub enum TriangleIntersection {
    Segment { start: Vec3, end: Vec3 },
    Coplanar { polygon: Vec<Vec3> },
}

pub fn triangle_normal(triangle: [Vec3; 3]) -> Vec3 {
    let [a, b, c] = triangle;
    (b - a).cross(c - a).normalize_or_zero()
}

pub fn triangle_area(triangle: [Vec3; 3]) -> f32 {
    let [a, b, c] = triangle;
    (b - a).cross(c - a).length() * 0.5
}

pub fn barycentric_weights(point: Vec3, triangle: [Vec3; 3]) -> Vec3 {
    let [a, b, c] = triangle.map(Vec3::as_dvec3);
    let (v0, v1, v2) = (b - a, c - a, point.as_dvec3() - a);
    let (d00, d01, d11) = (v0.dot(v0), v0.dot(v1), v1.dot(v1));
    let (d20, d21) = (v2.dot(v0), v2.dot(v1));
    let denominator = d00 * d11 - d01 * d01;
    if denominator > f64::EPSILON * d00 * d11 && denominator.is_finite() {
        let v = (d11 * d20 - d01 * d21) / denominator;
        let w = (d00 * d21 - d01 * d20) / denominator;
        return DVec3::new(1.0 - v - w, v, w).as_vec3();
    }
    degenerate_weights(point, triangle)
}

fn degenerate_weights(point: Vec3, triangle: [Vec3; 3]) -> Vec3 {
    let [start, end] = segment_hull(triangle);
    let (first, second) = (start.1, end.1);
    let mut weights = [0.0f32; 3];
    if first == second {
        weights[first] = 1.0;
        return Vec3::from_array(weights);
    }
    let closest = closest_point_on_segment(point, start.0, end.0);
    weights[first] = 1.0 - closest.parameter;
    weights[second] = closest.parameter;
    Vec3::from_array(weights)
}

fn segment_hull(triangle: [Vec3; 3]) -> [(Vec3, usize); 2] {
    let length = |(i, j): (usize, usize)| {
        triangle[i]
            .as_dvec3()
            .distance_squared(triangle[j].as_dvec3())
    };
    let (first, second) =
        [(1usize, 2usize), (2, 0)]
            .into_iter()
            .fold((0usize, 1usize), |longest, pair| {
                match length(pair) > length(longest) {
                    true => pair,
                    false => longest,
                }
            });
    [(triangle[first], first), (triangle[second], second)]
}

pub fn triangle_intersects_aabb(triangle: [Vec3; 3], bounds: &Aabb) -> bool {
    if bounds.is_empty() || !Aabb::from_triangle(triangle).intersects(bounds) {
        return false;
    }
    let center = bounds.center();
    let half = bounds.size() * 0.5;
    let vertices = triangle.map(|vertex| vertex - center);
    let edges = [
        vertices[1] - vertices[0],
        vertices[2] - vertices[1],
        vertices[0] - vertices[2],
    ];
    let separated = |axis: Vec3| {
        let projections = vertices.map(|vertex| vertex.dot(axis));
        let radius = half.dot(axis.abs());
        let low = projections[0].min(projections[1]).min(projections[2]);
        let high = projections[0].max(projections[1]).max(projections[2]);
        low > radius || high < -radius
    };
    for edge in edges {
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            let cross = axis.cross(edge);
            if cross != Vec3::ZERO && separated(cross) {
                return false;
            }
        }
    }
    let normal = edges[0].cross(edges[1]);
    normal == Vec3::ZERO || !separated(normal)
}

fn side(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f64 {
    -orient3d(a, b, c, d)
}

fn sign(value: f64) -> i8 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

fn is_degenerate(triangle: [Vec3; 3]) -> bool {
    let [a, b, c] = triangle;
    let xy = |v: Vec3| Vec2::new(v.x, v.y);
    let yz = |v: Vec3| Vec2::new(v.y, v.z);
    let zx = |v: Vec3| Vec2::new(v.z, v.x);
    orient2d(xy(a), xy(b), xy(c)) == 0.0
        && orient2d(yz(a), yz(b), yz(c)) == 0.0
        && orient2d(zx(a), zx(b), zx(c)) == 0.0
}

pub fn intersect_triangles(first: [Vec3; 3], second: [Vec3; 3]) -> Option<TriangleIntersection> {
    match (is_degenerate(first), is_degenerate(second)) {
        (true, true) => {
            let [start, end] = segment_hull(first).map(|(point, _)| point);
            let other = segment_hull(second).map(|(point, _)| point);
            intersect_segments_3d([start, end], other)
        }
        (true, false) => {
            intersect_segment_triangle(segment_hull(first).map(|(point, _)| point), second)
        }
        (false, true) => {
            intersect_segment_triangle(segment_hull(second).map(|(point, _)| point), first)
        }
        (false, false) => intersect_proper(first, second),
    }
}

#[derive(Clone, Copy)]
struct Feature {
    from: Vec3,
    to: Vec3,
    from_side: f64,
    to_side: f64,
}

impl Feature {
    fn point(&self) -> Vec3 {
        if self.from_side == 0.0 {
            return self.from;
        }
        let factor = (self.from_side / (self.from_side - self.to_side)).clamp(0.0, 1.0);
        let (from, to) = (self.from.as_dvec3(), self.to.as_dvec3());
        (from + (to - from) * factor).as_vec3()
    }
}

struct Section {
    start: Feature,
    end: Feature,
}

fn section(triangle: [Vec3; 3], sides: [f64; 3]) -> Section {
    let signs = sides.map(sign);
    let alone = (0..3).find(|&index| {
        let own = signs[index];
        own != 0
            && [signs[(index + 1) % 3], signs[(index + 2) % 3]]
                .iter()
                .all(|&other| other == 0 || other == -own)
    });
    match alone {
        Some(index) => {
            let (p, q, r) = (index, (index + 1) % 3, (index + 2) % 3);
            let feature = |from: usize| Feature {
                from: triangle[from],
                to: triangle[p],
                from_side: sides[from],
                to_side: sides[p],
            };
            match signs[p] > 0 {
                true => Section {
                    start: feature(r),
                    end: feature(q),
                },
                false => Section {
                    start: feature(q),
                    end: feature(r),
                },
            }
        }
        None => {
            let zero = (0..3)
                .find(|&index| signs[index] == 0)
                .expect("touching triangle has a vertex on the plane");
            let off = (zero + 1) % 3;
            let feature = Feature {
                from: triangle[zero],
                to: triangle[off],
                from_side: 0.0,
                to_side: sides[off],
            };
            Section {
                start: feature,
                end: feature,
            }
        }
    }
}

fn order(first: &Feature, second: &Feature) -> i8 {
    sign(side(first.from, first.to, second.from, second.to))
        * sign(first.to_side)
        * sign(second.to_side)
}

fn intersect_proper(first: [Vec3; 3], second: [Vec3; 3]) -> Option<TriangleIntersection> {
    let [p2, q2, r2] = second;
    let first_sides = first.map(|vertex| side(p2, q2, r2, vertex));
    let first_signs = first_sides.map(sign);
    if first_signs.iter().all(|&value| value > 0) || first_signs.iter().all(|&value| value < 0) {
        return None;
    }
    if first_signs.iter().all(|&value| value == 0) {
        return intersect_coplanar(first, second);
    }
    let [p1, q1, r1] = first;
    let second_sides = second.map(|vertex| side(p1, q1, r1, vertex));
    let second_signs = second_sides.map(sign);
    if second_signs.iter().all(|&value| value > 0) || second_signs.iter().all(|&value| value < 0) {
        return None;
    }
    let first_section = section(first, first_sides);
    let mut second_section = section(second, second_sides);
    std::mem::swap(&mut second_section.start, &mut second_section.end);
    if order(&first_section.start, &second_section.end) < 0
        || order(&first_section.end, &second_section.start) > 0
    {
        return None;
    }
    let start = match order(&first_section.start, &second_section.start) >= 0 {
        true => second_section.start.point(),
        false => first_section.start.point(),
    };
    let end = match order(&first_section.end, &second_section.end) >= 0 {
        true => first_section.end.point(),
        false => second_section.end.point(),
    };
    Some(TriangleIntersection::Segment { start, end })
}

fn dominant_axis(normal: DVec3) -> usize {
    let magnitude = normal.abs();
    if magnitude.x >= magnitude.y && magnitude.x >= magnitude.z {
        0
    } else if magnitude.y >= magnitude.z {
        1
    } else {
        2
    }
}

fn flat_axes(dropped: usize) -> (usize, usize) {
    ((dropped + 1) % 3, (dropped + 2) % 3)
}

fn exact_normal(triangle: [Vec3; 3]) -> DVec3 {
    let [a, b, c] = triangle.map(Vec3::as_dvec3);
    (b - a).cross(c - a)
}

#[derive(Clone, Copy)]
struct ClipVertex {
    point: DVec3,
    flat: DVec2,
    original: Option<Vec2>,
}

fn intersect_coplanar(first: [Vec3; 3], second: [Vec3; 3]) -> Option<TriangleIntersection> {
    let (u, v) = flat_axes(dominant_axis(exact_normal(first)));
    let flatten = |vertex: Vec3| Vec2::new(vertex[u], vertex[v]);
    let polygon: SmallVec<[ClipVertex; 9]> = first
        .iter()
        .map(|&vertex| ClipVertex {
            point: vertex.as_dvec3(),
            flat: flatten(vertex).as_dvec2(),
            original: Some(flatten(vertex)),
        })
        .collect();
    let mut clip = second.map(flatten);
    if orient2d(clip[0], clip[1], clip[2]) < 0.0 {
        clip.swap(1, 2);
    }
    let clipped = clip_by_triangle(polygon, clip)?;
    let mut points: Vec<Vec3> = Vec::with_capacity(clipped.len());
    for vertex in clipped {
        let point = vertex.point.as_vec3();
        if points.last() != Some(&point) && (points.is_empty() || points[0] != point) {
            points.push(point);
        }
    }
    Some(TriangleIntersection::Coplanar { polygon: points })
}

fn clip_by_triangle(
    mut polygon: SmallVec<[ClipVertex; 9]>,
    clip: [Vec2; 3],
) -> Option<SmallVec<[ClipVertex; 9]>> {
    for index in 0..3 {
        let (edge_start, edge_end) = (clip[index], clip[(index + 1) % 3]);
        let orientation = |vertex: &ClipVertex| match vertex.original {
            Some(flat) => orient2d(edge_start, edge_end, flat),
            None => {
                let (start, end) = (edge_start.as_dvec2(), edge_end.as_dvec2());
                (end - start).perp_dot(vertex.flat - start)
            }
        };
        let count = polygon.len();
        let closed = count > 2;
        let mut next: SmallVec<[ClipVertex; 9]> = SmallVec::new();
        if count == 1 {
            if orientation(&polygon[0]) >= 0.0 {
                next.push(polygon[0]);
            }
        } else {
            let edges = if closed { count } else { 1 };
            for edge in 0..edges {
                let start = polygon[edge];
                let end = polygon[(edge + 1) % count];
                let start_side = orientation(&start);
                let end_side = orientation(&end);
                if start_side >= 0.0 {
                    next.push(start);
                }
                if (start_side > 0.0 && end_side < 0.0) || (start_side < 0.0 && end_side > 0.0) {
                    let factor = start_side / (start_side - end_side);
                    next.push(ClipVertex {
                        point: start.point + (end.point - start.point) * factor,
                        flat: start.flat + (end.flat - start.flat) * factor,
                        original: None,
                    });
                }
                if !closed && end_side >= 0.0 {
                    next.push(end);
                }
            }
        }
        if next.is_empty() {
            return None;
        }
        polygon = next;
    }
    Some(polygon)
}

fn intersect_segment_triangle(
    segment: [Vec3; 2],
    triangle: [Vec3; 3],
) -> Option<TriangleIntersection> {
    let [a, b, c] = triangle;
    let [start, end] = segment;
    let start_side = side(a, b, c, start);
    let end_side = side(a, b, c, end);
    if (start_side > 0.0 && end_side > 0.0) || (start_side < 0.0 && end_side < 0.0) {
        return None;
    }
    if start_side == 0.0 && end_side == 0.0 {
        let (u, v) = flat_axes(dominant_axis(exact_normal(triangle)));
        let flatten = |vertex: Vec3| Vec2::new(vertex[u], vertex[v]);
        let polygon: SmallVec<[ClipVertex; 9]> = segment
            .iter()
            .map(|&vertex| ClipVertex {
                point: vertex.as_dvec3(),
                flat: flatten(vertex).as_dvec2(),
                original: Some(flatten(vertex)),
            })
            .collect();
        let mut clip = triangle.map(flatten);
        if orient2d(clip[0], clip[1], clip[2]) < 0.0 {
            clip.swap(1, 2);
        }
        let clipped = clip_by_triangle(polygon, clip)?;
        let first = clipped[0].point.as_vec3();
        let last = clipped[clipped.len() - 1].point.as_vec3();
        return Some(TriangleIntersection::Segment {
            start: first,
            end: last,
        });
    }
    let edge_signs = [
        sign(orient3d(start, end, a, b)),
        sign(orient3d(start, end, b, c)),
        sign(orient3d(start, end, c, a)),
    ];
    if edge_signs.contains(&1) && edge_signs.contains(&-1) {
        return None;
    }
    let crossing = Feature {
        from: start,
        to: end,
        from_side: start_side,
        to_side: end_side,
    };
    let point = match end_side == 0.0 {
        true => end,
        false => crossing.point(),
    };
    Some(TriangleIntersection::Segment {
        start: point,
        end: point,
    })
}

fn intersect_segments_3d(first: [Vec3; 2], second: [Vec3; 2]) -> Option<TriangleIntersection> {
    let [a0, a1] = first;
    let [b0, b1] = second;
    if orient3d(a0, a1, b0, b1) != 0.0 {
        return None;
    }
    let (start, end) = (a0.as_dvec3(), a1.as_dvec3());
    let normals = [
        (end - start).cross(b0.as_dvec3() - start),
        (end - start).cross(b1.as_dvec3() - start),
        (b1.as_dvec3() - b0.as_dvec3()).cross(start - b0.as_dvec3()),
    ];
    let normal = normals
        .into_iter()
        .max_by(|left, right| left.length_squared().total_cmp(&right.length_squared()))
        .expect("three candidate normals");
    let (u, v) = match normal == DVec3::ZERO {
        true => {
            let direction = match (end - start).length_squared()
                >= (b1.as_dvec3() - b0.as_dvec3()).length_squared()
            {
                true => end - start,
                false => b1.as_dvec3() - b0.as_dvec3(),
            };
            let kept = dominant_axis(direction);
            (kept, (kept + 1) % 3)
        }
        false => flat_axes(dominant_axis(normal)),
    };
    let flatten = |vertex: Vec3| Vec2::new(vertex[u], vertex[v]);
    let lift = |segment: [Vec3; 2], parameter: f32| segment[0].lerp(segment[1], parameter);
    match intersect_segments_2d([flatten(a0), flatten(a1)], [flatten(b0), flatten(b1)])? {
        SegmentIntersection2d::Point(crossing) => {
            let point = lift(first, crossing.first);
            Some(TriangleIntersection::Segment {
                start: point,
                end: point,
            })
        }
        SegmentIntersection2d::Overlap { start, end } => Some(TriangleIntersection::Segment {
            start: lift(first, start.first),
            end: lift(first, end.first),
        }),
    }
}
