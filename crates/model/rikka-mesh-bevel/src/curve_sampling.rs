use glam::Vec2;

use crate::curve_profile::{HandleType, ProfilePoint};
use crate::math::angle_2d;

const TABLE_RESOLUTION: usize = 16;
const TABLE_MAX: usize = 512;
const AUTO_HANDLE_FACTOR: f32 = 2.5614;

fn computed_handles(points: &[ProfilePoint]) -> Vec<[Vec2; 2]> {
    let count = points.len();
    (0..count)
        .map(|i| {
            let previous = (i > 0).then(|| points[i - 1].position);
            let next = (i + 1 < count).then(|| points[i + 1].position);
            point_handles(&points[i], previous, next)
        })
        .collect()
}

fn point_handles(point: &ProfilePoint, previous: Option<Vec2>, next: Option<Vec2>) -> [Vec2; 2] {
    let [h1, h2] = point.handle_types;
    let mut handles = point.handle_positions;
    if h1 == HandleType::Free && h2 == HandleType::Free {
        return handles;
    }
    let location = point.position;
    let previous = previous.unwrap_or_else(|| {
        let next = next.unwrap_or(location);
        2.0 * location - next
    });
    let next = next.unwrap_or(2.0 * location - previous);
    let dvec_a = location - previous;
    let dvec_b = next - location;
    let mut len_a = dvec_a.length();
    let mut len_b = dvec_b.length();
    if len_a == 0.0 {
        len_a = 1.0;
    }
    if len_b == 0.0 {
        len_b = 1.0;
    }
    if h1 == HandleType::Auto || h2 == HandleType::Auto {
        let tangent = Vec2::new(
            dvec_b.x / len_b + dvec_a.x / len_a,
            dvec_b.y / len_b + dvec_a.y / len_a,
        );
        let length = tangent.length() * AUTO_HANDLE_FACTOR;
        if length != 0.0 {
            if h1 == HandleType::Auto {
                handles[0] = location + tangent * -(len_a / length);
            }
            if h2 == HandleType::Auto {
                handles[1] = location + tangent * (len_b / length);
            }
        }
    }
    if h1 == HandleType::Vector {
        handles[0] = location + dvec_a * (-1.0 / 3.0);
    }
    if h2 == HandleType::Vector {
        handles[1] = location + dvec_b * (1.0 / 3.0);
    }
    handles
}

fn is_curved_edge(points: &[ProfilePoint], edge: usize) -> bool {
    points[edge].handle_types[1] != HandleType::Vector
        || points[edge + 1].handle_types[0] != HandleType::Vector
}

fn merge_sort<T: Copy>(items: &mut [T], before_or_tie: &impl Fn(&T, &T) -> bool) {
    let count = items.len();
    if count <= 1 {
        return;
    }
    let half = count / 2;
    merge_sort(&mut items[..half], before_or_tie);
    merge_sort(&mut items[half..], before_or_tie);
    let mut merged = Vec::with_capacity(count);
    let (mut left, mut right) = (0, half);
    while left < half && right < count {
        if before_or_tie(&items[left], &items[right]) {
            merged.push(items[left]);
            left += 1;
        } else {
            merged.push(items[right]);
            right += 1;
        }
    }
    merged.extend_from_slice(&items[left..half]);
    merged.extend_from_slice(&items[right..count]);
    items.copy_from_slice(&merged);
}

fn forward_difference(q: [f32; 4], steps: usize, out: &mut [f32]) {
    let mut f = steps as f32;
    let rt0 = q[0];
    let rt1 = 3.0 * (q[1] - q[0]) / f;
    f *= f;
    let rt2 = 3.0 * (q[0] - 2.0 * q[1] + q[2]) / f;
    f *= steps as f32;
    let rt3 = (q[3] - q[0] + 3.0 * (q[1] - q[2])) / f;
    let mut q0 = rt0;
    let mut q1 = rt1 + rt2 + rt3;
    let mut q2 = 2.0 * rt2 + 6.0 * rt3;
    let q3 = 6.0 * rt3;
    for slot in out.iter_mut().take(steps + 1) {
        *slot = q0;
        q0 += q1;
        q1 += q2;
        q2 += q3;
    }
}

fn sample_counts(
    points: &[ProfilePoint],
    handles: &[[Vec2; 2]],
    segments: usize,
    sample_straight_edges: bool,
) -> Vec<usize> {
    let edges = points.len() - 1;
    let curvature: Vec<f32> = (0..edges)
        .map(|i| {
            angle_2d(
                handles[i][1] - points[i].position,
                points[i + 1].position - handles[i + 1][0],
            )
        })
        .collect();
    let mut sorted: Vec<usize> = (0..edges).collect();
    merge_sort(&mut sorted, &|&a, &b| curvature[a] > curvature[b]);
    let mut counts = vec![0usize; edges];
    let mut left;
    if segments >= edges {
        if sample_straight_edges {
            let common = segments / edges;
            left = segments % edges;
            if common > 0 {
                counts.iter_mut().for_each(|count| *count = common);
            }
        } else {
            let curved = (0..edges).filter(|&i| is_curved_edge(points, i)).count();
            let curved = match curved {
                0 => edges,
                count => count,
            };
            left = segments - (edges - curved);
            let common = left / curved;
            if common > 0 {
                for (i, count) in counts.iter_mut().enumerate() {
                    if is_curved_edge(points, i) || curved == edges {
                        *count += common;
                    } else {
                        *count = 1;
                    }
                }
            }
            left -= common * curved;
        }
    } else {
        left = segments;
    }
    for &edge in sorted.iter().take(left) {
        counts[edge] += 1;
    }
    counts
}

pub(crate) fn path_samples(
    points: &[ProfilePoint],
    segments: usize,
    sample_straight_edges: bool,
) -> Vec<Vec2> {
    let handles = computed_handles(points);
    let counts = sample_counts(points, &handles, segments, sample_straight_edges);
    let mut xs = vec![0.0f32; segments + 1];
    let mut ys = vec![0.0f32; segments + 1];
    let mut at = 0;
    for (edge, &count) in counts.iter().enumerate() {
        if count > 0 {
            let (start, end) = (points[edge], points[edge + 1]);
            forward_difference(
                [
                    start.position.x,
                    handles[edge][1].x,
                    handles[edge + 1][0].x,
                    end.position.x,
                ],
                count,
                &mut xs[at..],
            );
            forward_difference(
                [
                    start.position.y,
                    handles[edge][1].y,
                    handles[edge + 1][0].y,
                    end.position.y,
                ],
                count,
                &mut ys[at..],
            );
        }
        at += count;
    }
    xs.into_iter()
        .zip(ys)
        .map(|(x, y)| Vec2::new(x, y))
        .collect()
}

fn table(points: &[ProfilePoint]) -> Vec<Vec2> {
    let size = ((points.len() - 1) * TABLE_RESOLUTION + 1).clamp(1, TABLE_MAX);
    let mut table = match size > 1 {
        true => path_samples(points, size - 1, false),
        false => vec![Vec2::ZERO],
    };
    table.resize(size + 1, Vec2::ZERO);
    table[size - 1] = Vec2::new(0.0, 1.0);
    table
}

pub(crate) fn even_length_samples(points: &[ProfilePoint], segments: usize) -> Vec<Vec2> {
    let table = table(points);
    let size = ((points.len() - 1) * TABLE_RESOLUTION + 1).clamp(1, TABLE_MAX);
    let step_length = |i: usize| -> f32 {
        match i + 1 < table.len() {
            true => table[i].distance(table[i + 1]),
            false => 0.0,
        }
    };
    let total: f32 = (0..size - 1).map(step_length).sum();
    let segment_length = total / segments as f32;
    let mut samples = vec![Vec2::ZERO; segments + 1];
    samples[0] = table[0];
    let mut to_next = step_length(0);
    let mut from_previous = 0.0f32;
    let mut index = 0usize;
    let mut segment_left = segment_length;
    for sample in samples.iter_mut().take(segments).skip(1) {
        while to_next < segment_left && index + 1 < size {
            segment_left -= to_next;
            index += 1;
            to_next = step_length(index);
            from_previous = 0.0;
        }
        let factor = (from_previous + segment_left) / (from_previous + to_next);
        let next = table[(index + 1).min(table.len() - 1)];
        let current = table[index];
        *sample = Vec2::new(
            factor * next.x + (1.0 - factor) * current.x,
            factor * next.y + (1.0 - factor) * current.y,
        );
        to_next -= segment_left;
        from_previous += segment_left;
        segment_left = segment_length;
    }
    samples
}
