use glam::{DVec2, Vec2, Vec3};

use crate::closest::closest_point_on_segment;
use crate::predicates::orient2d;

const WEIGHT_EPSILON_SCALE: f32 = 16.0 * f32::EPSILON;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FillRule {
    EvenOdd,
    NonZero,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentCrossing {
    pub point: Vec2,
    pub first: f32,
    pub second: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SegmentIntersection2d {
    Point(SegmentCrossing),
    Overlap {
        start: SegmentCrossing,
        end: SegmentCrossing,
    },
}

pub fn winding_number(point: Vec2, polygon: &[Vec2]) -> i32 {
    let mut winding = 0;
    for (index, &start) in polygon.iter().enumerate() {
        let end = polygon[(index + 1) % polygon.len()];
        if start.y <= point.y {
            if end.y > point.y && orient2d(start, end, point) > 0.0 {
                winding += 1;
            }
        } else if end.y <= point.y && orient2d(start, end, point) < 0.0 {
            winding -= 1;
        }
    }
    winding
}

pub fn point_in_polygon(point: Vec2, polygon: &[Vec2], rule: FillRule) -> bool {
    let winding = winding_number(point, polygon);
    match rule {
        FillRule::EvenOdd => winding % 2 != 0,
        FillRule::NonZero => winding != 0,
    }
}

pub fn intersect_segments_2d(first: [Vec2; 2], second: [Vec2; 2]) -> Option<SegmentIntersection2d> {
    let [a0, a1] = first;
    let [b0, b1] = second;
    let side_a0 = orient2d(b0, b1, a0);
    let side_a1 = orient2d(b0, b1, a1);
    let side_b0 = orient2d(a0, a1, b0);
    let side_b1 = orient2d(a0, a1, b1);
    if side_a0 == 0.0 && side_a1 == 0.0 && side_b0 == 0.0 && side_b1 == 0.0 {
        return collinear_overlap(first, second);
    }
    let separated = |p: f64, q: f64| (p > 0.0 && q > 0.0) || (p < 0.0 && q < 0.0);
    if separated(side_a0, side_a1) || separated(side_b0, side_b1) {
        return None;
    }
    let first_parameter = (side_a0 / (side_a0 - side_a1)).clamp(0.0, 1.0);
    let second_parameter = (side_b0 / (side_b0 - side_b1)).clamp(0.0, 1.0);
    let point = if side_a0 == 0.0 {
        a0
    } else if side_a1 == 0.0 {
        a1
    } else if side_b0 == 0.0 {
        b0
    } else if side_b1 == 0.0 {
        b1
    } else {
        lerp2(a0, a1, first_parameter)
    };
    Some(SegmentIntersection2d::Point(SegmentCrossing {
        point,
        first: first_parameter as f32,
        second: second_parameter as f32,
    }))
}

fn lerp2(start: Vec2, end: Vec2, parameter: f64) -> Vec2 {
    (start.as_dvec2() + (end.as_dvec2() - start.as_dvec2()) * parameter).as_vec2()
}

fn parameter_on(start: Vec2, end: Vec2, point: Vec2) -> f64 {
    let direction = end.as_dvec2() - start.as_dvec2();
    let length_squared = direction.length_squared();
    match length_squared == 0.0 {
        true => 0.0,
        false => {
            ((point.as_dvec2() - start.as_dvec2()).dot(direction) / length_squared).clamp(0.0, 1.0)
        }
    }
}

fn collinear_overlap(first: [Vec2; 2], second: [Vec2; 2]) -> Option<SegmentIntersection2d> {
    let [a0, a1] = first.map(|point| point.as_dvec2());
    let [b0, b1] = second.map(|point| point.as_dvec2());
    let first_direction = a1 - a0;
    let second_direction = b1 - b0;
    let axis = match first_direction.length_squared() >= second_direction.length_squared() {
        true => first_direction,
        false => second_direction,
    };
    if axis == DVec2::ZERO {
        return (a0 == b0).then_some(SegmentIntersection2d::Point(SegmentCrossing {
            point: first[0],
            first: 0.0,
            second: 0.0,
        }));
    }
    let project = |point: DVec2| point.dot(axis);
    let (first_low, first_high) = ordered(project(a0), project(a1));
    let (second_low, second_high) = ordered(project(b0), project(b1));
    let low = first_low.max(second_low);
    let high = first_high.min(second_high);
    if low > high {
        return None;
    }
    let crossing = |value: f64| {
        let point = match value == project(a0) {
            true => a0,
            false => match value == project(a1) {
                true => a1,
                false => match value == project(b0) {
                    true => b0,
                    false => b1,
                },
            },
        };
        let point = point.as_vec2();
        SegmentCrossing {
            point,
            first: parameter_on(first[0], first[1], point) as f32,
            second: parameter_on(second[0], second[1], point) as f32,
        }
    };
    let start = crossing(low);
    match low == high {
        true => Some(SegmentIntersection2d::Point(start)),
        false => Some(SegmentIntersection2d::Overlap {
            start,
            end: crossing(high),
        }),
    }
}

fn ordered(first: f64, second: f64) -> (f64, f64) {
    (first.min(second), first.max(second))
}

pub fn signed_area_2d(polygon: &[Vec2]) -> f32 {
    let mut twice = 0.0f64;
    for (index, &start) in polygon.iter().enumerate() {
        let end = polygon[(index + 1) % polygon.len()];
        twice += f64::from(start.x) * f64::from(end.y) - f64::from(end.x) * f64::from(start.y);
    }
    (twice * 0.5) as f32
}

pub fn polygon_normal(polygon: &[Vec3]) -> Vec3 {
    let Some(&origin) = polygon.first() else {
        return Vec3::ZERO;
    };
    let mut normal = Vec3::ZERO;
    let mut previous = polygon[polygon.len() - 1] - origin;
    for &vertex in polygon {
        let current = vertex - origin;
        normal += Vec3::new(
            (previous.y - current.y) * (previous.z + current.z),
            (previous.z - current.z) * (previous.x + current.x),
            (previous.x - current.x) * (previous.y + current.y),
        );
        previous = current;
    }
    normal * 0.5
}

pub fn polygon_area(polygon: &[Vec3]) -> f32 {
    polygon_normal(polygon).length()
}

pub fn signed_volume(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f32 {
    (b - a).dot((c - a).cross(d - a)) / 6.0
}

pub fn signed_mesh_volume(triangles: impl IntoIterator<Item = [Vec3; 3]>) -> f32 {
    let mut triangles = triangles.into_iter().peekable();
    let Some(origin) = triangles.peek().map(|triangle| triangle[0]) else {
        return 0.0;
    };
    let volume: f64 = triangles
        .map(|[a, b, c]| {
            let (a, b, c) = (
                (a - origin).as_dvec3(),
                (b - origin).as_dvec3(),
                (c - origin).as_dvec3(),
            );
            a.dot(b.cross(c))
        })
        .sum();
    (volume / 6.0) as f32
}

pub fn mean_value_weights(polygon: &[Vec3], point: Vec3, weights: &mut [f32]) {
    assert_eq!(
        polygon.len(),
        weights.len(),
        "one weight per polygon vertex"
    );
    let count = polygon.len();
    if count == 0 {
        return;
    }
    let max_offset = polygon
        .iter()
        .map(|&vertex| (vertex - point).abs().max_element())
        .fold(0.0f32, f32::max);
    let epsilon = WEIGHT_EPSILON_SCALE * max_offset;
    let epsilon_squared = epsilon * epsilon;
    let half_tangent = |current: Vec3, next: Vec3, current_length: f32, next_length: f32| -> f32 {
        let area = current.cross(next).length();
        if area != 0.0 {
            let result = (current_length * next_length - current.dot(next)) / area;
            if result.is_finite() {
                return result;
            }
        }
        0.0
    };
    let offset_from = |index: usize| {
        let offset = polygon[index] - point;
        (offset, offset.length())
    };
    let mut current_index = count - 1;
    let mut next_index = 0;
    let (mut current, mut current_length) = offset_from((count + count - 2) % count);
    let (mut next, mut next_length) = offset_from(current_index);
    let mut previous_tangent = half_tangent(current, next, current_length, next_length);
    let mut total = 0.0f32;
    let mut snapped = None;
    while next_index < count {
        if next_length < epsilon {
            snapped = Some(Snap::Vertex(current_index));
            break;
        }
        let closest = closest_point_on_segment(point, polygon[current_index], polygon[next_index]);
        if closest.point.distance_squared(point) < epsilon_squared {
            snapped = Some(Snap::Edge(current_index, next_index));
            break;
        }
        current = next;
        current_length = next_length;
        (next, next_length) = offset_from(next_index);
        let tangent = half_tangent(current, next, current_length, next_length);
        weights[current_index] = (previous_tangent + tangent) / current_length;
        total += weights[current_index];
        current_index = next_index;
        next_index += 1;
        previous_tangent = tangent;
    }
    finish_weights(weights, total, snapped, |start, end| {
        line_factor(point, polygon[start], polygon[end])
    });
}

pub fn mean_value_weights_2d(polygon: &[Vec2], point: Vec2, weights: &mut [f32]) {
    assert_eq!(
        polygon.len(),
        weights.len(),
        "one weight per polygon vertex"
    );
    let count = polygon.len();
    if count == 0 {
        return;
    }
    let max_offset = polygon
        .iter()
        .map(|&vertex| (vertex - point).abs().max_element())
        .fold(0.0f32, f32::max);
    let epsilon = WEIGHT_EPSILON_SCALE * max_offset;
    let epsilon_squared = epsilon * epsilon;
    let half_tangent =
        |current: DVec2, next: DVec2, current_length: f64, next_length: f64| -> f64 {
            let area = current.perp_dot(next);
            if area != 0.0 {
                let result = (current_length * next_length - current.dot(next)) / area;
                if result.is_finite() {
                    return result;
                }
            }
            0.0
        };
    let offset_from = |index: usize| {
        let offset = polygon[index].as_dvec2() - point.as_dvec2();
        (offset, offset.length())
    };
    let mut current_index = count - 1;
    let mut next_index = 0;
    let (mut current, mut current_length) = offset_from((count + count - 2) % count);
    let (mut next, mut next_length) = offset_from(current_index);
    let mut previous_tangent = half_tangent(current, next, current_length, next_length);
    let mut total = 0.0f32;
    let mut snapped = None;
    while next_index < count {
        if next_length < f64::from(epsilon) {
            snapped = Some(Snap::Vertex(current_index));
            break;
        }
        if distance_squared_to_segment_2d(point, polygon[current_index], polygon[next_index])
            < epsilon_squared
        {
            snapped = Some(Snap::Edge(current_index, next_index));
            break;
        }
        current = next;
        current_length = next_length;
        (next, next_length) = offset_from(next_index);
        let tangent = half_tangent(current, next, current_length, next_length);
        weights[current_index] = match current_length == 0.0 {
            true => 0.0,
            false => ((previous_tangent + tangent) / current_length) as f32,
        };
        total += weights[current_index];
        current_index = next_index;
        next_index += 1;
        previous_tangent = tangent;
    }
    finish_weights(weights, total, snapped, |start, end| {
        line_factor_2d(point, polygon[start], polygon[end])
    });
}

#[derive(Clone, Copy)]
enum Snap {
    Vertex(usize),
    Edge(usize, usize),
}

fn finish_weights(
    weights: &mut [f32],
    total: f32,
    snapped: Option<Snap>,
    factor: impl Fn(usize, usize) -> f32,
) {
    match snapped {
        Some(Snap::Vertex(index)) => {
            weights.fill(0.0);
            weights[index] = 1.0;
        }
        Some(Snap::Edge(start, end)) => {
            weights.fill(0.0);
            let factor = factor(start, end).clamp(0.0, 1.0);
            weights[start] = 1.0 - factor;
            weights[end] = factor;
        }
        None => {
            if total != 0.0 {
                weights.iter_mut().for_each(|weight| *weight /= total);
            }
        }
    }
}

fn line_factor(point: Vec3, start: Vec3, end: Vec3) -> f32 {
    let direction = end - start;
    let length_squared = direction.length_squared();
    match length_squared > 0.0 {
        true => direction.dot(point - start) / length_squared,
        false => 0.0,
    }
}

fn line_factor_2d(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let direction = end - start;
    let length_squared = direction.length_squared();
    match length_squared > 0.0 {
        true => direction.dot(point - start) / length_squared,
        false => 0.0,
    }
}

fn distance_squared_to_segment_2d(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let factor = line_factor_2d(point, start, end);
    let closest = match factor <= 0.0 {
        true => start,
        false => match factor >= 1.0 {
            true => end,
            false => start + (end - start) * factor,
        },
    };
    closest.distance_squared(point)
}
