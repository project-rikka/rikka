use std::ops::Range;
use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3};
use rikka_polymesh::{MergeData, Polymesh};

use super::Scope;
use crate::kinds::array::{Array, FitType};
use crate::note::NoteKind;
use crate::origins;
use crate::space::inverse;

const MAX_VERTICES: usize = 1 << 26;
const UNMAPPED: i64 = -1;
const FIT_EPSILON: f32 = 1.0e-6;
const UNIT_SCALE_EPSILON: f32 = 1.0e-6;

pub(crate) fn evaluate(array: &Array, mesh: &mut Polymesh, scope: &mut Scope<'_>) {
    let ctx = scope.ctx;
    let mut offset = Mat4::IDENTITY;
    if array.use_constant_offset {
        offset.w_axis += array.constant_offset_displace.extend(0.0);
    }
    if array.use_relative_offset && !mesh.positions.is_empty() {
        let (low, high) = bounds(&mesh.positions);
        offset.w_axis += (array.relative_offset_displace * (high - low)).extend(0.0);
    }
    if array.use_object_offset
        && let Some(matrix) = array.offset_object.and_then(|object| ctx.matrix(object))
    {
        offset = offset * inverse(ctx.object_matrix()) * matrix;
    }
    let scale = Vec3::new(
        offset.x_axis.truncate().length(),
        offset.y_axis.truncate().length(),
        offset.z_axis.truncate().length(),
    );
    let offset_has_scale = (scale - Vec3::ONE).abs().max_element() > UNIT_SCALE_EPSILON;

    let chunk_vertices = mesh.positions.len();
    let cap = |object: Option<crate::reference::ObjectRef>| -> Option<Arc<Polymesh>> {
        let object = object?;
        if Some(object) == ctx.object() {
            return None;
        }
        ctx.mesh(object)
    };
    let start_cap = cap(array.start_cap);
    let end_cap = cap(array.end_cap);
    let cap_vertices = start_cap.as_ref().map_or(0, |cap| cap.positions.len())
        + end_cap.as_ref().map_or(0, |cap| cap.positions.len());

    let mut count = array.count as usize;
    if array.fit_type == FitType::FitLength {
        let distance = offset.w_axis.truncate().length();
        count = match distance > FIT_EPSILON {
            true => ((array.fit_length + FIT_EPSILON) / distance) as usize + 1,
            false => 1,
        };
    }
    if count
        .saturating_mul(chunk_vertices)
        .saturating_add(cap_vertices)
        > MAX_VERTICES
    {
        scope.note(NoteKind::CountReduced {
            requested: count.min(u32::MAX as usize) as u32,
            evaluated: 1,
        });
        count = 1;
    }
    let count = count.max(1);

    let merging = array.use_merge_vertices;
    let distance = array.merge_threshold;
    let chunk = mesh.clone();
    let mut result = std::mem::take(mesh);
    let mut doubles: Vec<i64> = Vec::new();
    let mut current = Mat4::IDENTITY;
    for copy in 1..count {
        current *= offset;
        let mut instance = chunk.clone();
        instance.transform_without_flip(&current);
        let shift = array.offset_uv * copy as f32;
        if shift != Vec2::ZERO {
            for name in instance.uv_maps.names.clone() {
                if let Some(uvs) = instance.uv_map_mut(&name) {
                    for uv in uvs {
                        *uv += shift;
                    }
                }
            }
        }
        result.append(&instance);
        if merging {
            doubles.resize(result.positions.len(), UNMAPPED);
            let this_start = copy * chunk_vertices;
            let previous_start = (copy - 1) * chunk_vertices;
            if !offset_has_scale && copy >= 2 {
                for vertex in 0..chunk_vertices {
                    let mut target = doubles[previous_start + vertex];
                    if target != UNMAPPED {
                        target += chunk_vertices as i64;
                        while target != UNMAPPED
                            && doubles[target as usize] != UNMAPPED
                            && doubles[target as usize] != target
                        {
                            let next = doubles[target as usize];
                            target = match result.positions[this_start + vertex]
                                .distance(result.positions[next as usize])
                                <= distance
                            {
                                true => next,
                                false => UNMAPPED,
                            };
                        }
                    }
                    doubles[this_start + vertex] = target;
                }
            } else {
                map_doubles(
                    &mut doubles,
                    &result.positions,
                    previous_start..previous_start + chunk_vertices,
                    this_start..this_start + chunk_vertices,
                    distance,
                );
            }
        }
    }
    let first = 0..chunk_vertices;
    let last = (count - 1) * chunk_vertices..count * chunk_vertices;
    if merging {
        doubles.resize(result.positions.len(), UNMAPPED);
        if array.use_merge_vertices_cap && count > 1 {
            map_doubles(
                &mut doubles,
                &result.positions,
                last.clone(),
                first.clone(),
                distance,
            );
        }
    }
    let caps = [
        (start_cap, inverse(offset), first),
        (end_cap, current * offset, last),
    ];
    for (cap, placement, neighbours) in caps {
        let Some(cap) = cap else {
            continue;
        };
        let mut cap = (*cap).clone();
        origins::strip(&mut cap);
        cap.transform_without_flip(&placement);
        let start = result.positions.len();
        let edges_start = result.edge_count();
        result.append(&cap);
        let edges_end = result.edge_count();
        origins::mark_control_range(&mut result, edges_start..edges_end);
        if merging {
            doubles.resize(result.positions.len(), UNMAPPED);
            map_doubles(
                &mut doubles,
                &result.positions,
                neighbours,
                start..result.positions.len(),
                distance,
            );
        }
    }
    result.ensure_edges();

    if merging {
        let mut targets: Vec<u32> = (0..result.positions.len() as u32).collect();
        let mut merged = false;
        for vertex in 0..doubles.len() {
            let mut target = doubles[vertex];
            if target == UNMAPPED {
                continue;
            }
            while doubles[target as usize] != UNMAPPED && doubles[target as usize] != target {
                target = doubles[target as usize];
            }
            if target as usize != vertex {
                targets[vertex] = target as u32;
                merged = true;
            }
        }
        if merged {
            result.merge_points(&targets, MergeData::KeepTarget);
        }
    }
    *mesh = result;
}

fn bounds(points: &[Vec3]) -> (Vec3, Vec3) {
    points.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(low, high), point| (low.min(*point), high.max(*point)),
    )
}

fn map_doubles(
    doubles: &mut [i64],
    positions: &[Vec3],
    targets: Range<usize>,
    sources: Range<usize>,
    distance: f32,
) {
    let reach = (3.0f32.sqrt() + 0.00005) * distance;
    let sorted = |range: Range<usize>| {
        let mut items: Vec<(usize, f32)> = range
            .map(|vertex| (vertex, positions[vertex].element_sum()))
            .collect();
        items.sort_by(|a, b| a.1.total_cmp(&b.1));
        items
    };
    let sorted_targets = sorted(targets);
    let sorted_sources = sorted(sources);
    let mut low = 0;
    for &(source, sum) in &sorted_sources {
        if doubles[source] != UNMAPPED {
            continue;
        }
        while low < sorted_targets.len() && sorted_targets[low].1 < sum - reach {
            low += 1;
        }
        if low >= sorted_targets.len() {
            break;
        }
        let mut best = UNMAPPED;
        let mut best_distance = distance * distance;
        let mut scan = low;
        while scan < sorted_targets.len() && sorted_targets[scan].1 <= sum + reach {
            let (target, _) = sorted_targets[scan];
            let squared = positions[source].distance_squared(positions[target]);
            if squared <= best_distance {
                best_distance = squared;
                best = target as i64;
                while best != UNMAPPED
                    && doubles[best as usize] != UNMAPPED
                    && doubles[best as usize] != best
                {
                    let next = doubles[best as usize];
                    best = match positions[source].distance(positions[next as usize]) <= distance {
                        true => next,
                        false => UNMAPPED,
                    };
                }
            }
            scan += 1;
        }
        doubles[source] = best;
    }
}
