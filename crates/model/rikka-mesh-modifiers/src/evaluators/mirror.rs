use glam::{Mat4, Vec2, Vec3};
use rikka_polymesh::{Influence, Influences, MergeData, Polymesh, VertexGroup};

use super::Scope;
use super::bisect::bisect;
use crate::kinds::mirror::Mirror;
use crate::note::SkipReason;
use crate::space::inverse;

pub(crate) fn evaluate(
    mirror: &Mirror,
    mesh: &mut Polymesh,
    scope: &Scope<'_>,
) -> Result<(), SkipReason> {
    let relative = mirror
        .mirror_object
        .and_then(|object| scope.ctx.matrix(object))
        .map(|matrix| inverse(matrix) * scope.ctx.object_matrix());
    for axis in 0..3 {
        if mirror.use_axis[axis] {
            mirror_axis(mirror, mesh, axis, relative)?;
        }
    }
    Ok(())
}

fn mirror_axis(
    mirror: &Mirror,
    mesh: &mut Polymesh,
    axis: usize,
    relative: Option<Mat4>,
) -> Result<(), SkipReason> {
    let mut reflect = Mat4::IDENTITY;
    reflect.col_mut(axis)[axis] = -1.0;
    let (matrix, plane_point, plane_normal) = match relative {
        Some(relative) => {
            let back = inverse(relative);
            let mut normal = back.col(axis).truncate();
            let scale = Vec3::new(
                relative.x_axis.truncate().length_squared(),
                relative.y_axis.truncate().length_squared(),
                relative.z_axis.truncate().length_squared(),
            );
            let largest = scale.max_element();
            if largest != 0.0 {
                normal *= scale / largest;
            }
            (back * reflect * relative, back.w_axis.truncate(), normal)
        }
        None => (reflect, Vec3::ZERO, -reflect.col(axis).truncate()),
    };
    if mirror.use_bisect_axis[axis] {
        let normal = match mirror.use_bisect_flip_axis[axis] {
            true => plane_normal,
            false => -plane_normal,
        };
        bisect(mesh, plane_point, normal, mirror.bisect_threshold)?;
    }

    let vertices = mesh.point_count();
    let merging = mirror.use_mirror_merge;
    let tolerance = mirror.merge_threshold * mirror.merge_threshold;
    let mut copy = mesh.clone();
    copy.transform_without_flip(&matrix);
    copy.flip_faces();
    let mut targets: Vec<u32> = (0..vertices as u32 * 2).collect();
    let mut merged = vec![false; vertices];
    if merging {
        for vertex in 0..vertices {
            if mesh.positions[vertex].distance_squared(copy.positions[vertex]) < tolerance {
                let middle = (mesh.positions[vertex] + copy.positions[vertex]) * 0.5;
                mesh.positions[vertex] = middle;
                copy.positions[vertex] = middle;
                merged[vertex] = true;
                match mirror.use_correct_order_on_merge {
                    true => targets[vertices + vertex] = vertex as u32,
                    false => targets[vertex] = (vertices + vertex) as u32,
                }
            }
        }
    }
    for name in copy.uv_maps.names.clone() {
        if let Some(uvs) = copy.uv_map_mut(&name) {
            for uv in uvs {
                *uv = mirror_uv(mirror, *uv);
            }
        }
    }
    if mesh.vertex_groups.has_weights() && mirror.use_mirror_vertex_groups {
        let flips = flip_map(&mesh.vertex_groups.groups);
        if flips.iter().any(Option::is_some) {
            for (vertex, &merged_here) in merged.iter().enumerate() {
                match (merged_here, mirror.use_correct_order_on_merge) {
                    (true, true) => flip_merged(&mut mesh.vertex_groups.weights[vertex], &flips),
                    (true, false) => flip_merged(&mut copy.vertex_groups.weights[vertex], &flips),
                    _ => flip(&mut copy.vertex_groups.weights[vertex], &flips),
                }
            }
        }
    }
    mesh.append(&copy);
    if merging
        && targets
            .iter()
            .enumerate()
            .any(|(index, &target)| index as u32 != target)
    {
        mesh.merge_points(&targets, MergeData::KeepTarget);
    }
    Ok(())
}

fn mirror_uv(mirror: &Mirror, uv: Vec2) -> Vec2 {
    let mut uv = uv;
    let flip = |value: f32, offset: f32| match mirror.use_mirror_udim {
        true => value.ceil() - value % 1.0 + offset,
        false => 1.0 - value + offset,
    };
    if mirror.use_mirror_u {
        uv.x = flip(uv.x, mirror.mirror_offset.x);
    }
    if mirror.use_mirror_v {
        uv.y = flip(uv.y, mirror.mirror_offset.y);
    }
    uv + mirror.offset
}

fn flip(influences: &mut Influences, flips: &[Option<u32>]) {
    for influence in influences.iter_mut() {
        if let Some(Some(other)) = flips.get(influence.group as usize) {
            influence.group = *other;
        }
    }
}

fn flip_merged(influences: &mut Influences, flips: &[Option<u32>]) {
    let count = influences.len();
    for index in 0..count {
        let influence = influences[index];
        let Some(Some(other)) = flips.get(influence.group as usize) else {
            continue;
        };
        let other_index = match influences
            .iter()
            .position(|candidate| candidate.group == *other)
        {
            Some(position) => position,
            None => {
                influences.push(Influence {
                    group: *other,
                    weight: 0.0,
                });
                influences.len() - 1
            }
        };
        let average = 0.5 * (influences[other_index].weight + influence.weight);
        influences[other_index].weight = average;
        influences[index].weight = average;
    }
}

fn flip_map(groups: &[VertexGroup]) -> Vec<Option<u32>> {
    let mut map = vec![None; groups.len()];
    for (index, group) in groups.iter().enumerate() {
        let flipped = flip_side_name(&group.name);
        if flipped != group.name
            && let Some(other) = groups
                .iter()
                .position(|candidate| candidate.name == flipped)
        {
            map[index] = Some(other as u32);
        }
    }
    map
}

fn flip_side_name(name: &str) -> String {
    let bytes = name.as_bytes();
    if bytes.len() < 3 {
        return name.to_owned();
    }
    let mut body = name;
    let mut number = "";
    if bytes[bytes.len() - 1].is_ascii_digit()
        && let Some(dot) = name.rfind('.')
        && name[dot + 1..].starts_with(|character: char| character.is_ascii_digit())
    {
        number = &name[dot..];
        body = &name[..dot];
    }
    let separator = |byte: u8| matches!(byte, b'.' | b' ' | b'-' | b'_');
    let swap = |byte: u8| match byte {
        b'l' => Some("r"),
        b'r' => Some("l"),
        b'L' => Some("R"),
        b'R' => Some("L"),
        _ => None,
    };
    let length = body.len();
    let body_bytes = body.as_bytes();
    if length > 1
        && separator(body_bytes[length - 2])
        && let Some(replace) = swap(body_bytes[length - 1])
    {
        return format!("{}{replace}{number}", &body[..length - 1]);
    }
    if length > 1
        && separator(body_bytes[1])
        && let Some(replace) = swap(body_bytes[0])
    {
        return format!("{replace}{}{number}", &body[1..]);
    }
    if length > 5 {
        let lower = body.to_ascii_lowercase();
        for (word, other) in [("right", "left"), ("left", "right")] {
            if let Some(at) = lower.find(word)
                && (at == 0 || at == length - word.len())
            {
                let matched = &body_bytes[at..at + word.len()];
                let replace = if matched[0].is_ascii_lowercase() {
                    other.to_owned()
                } else if matched[1].is_ascii_uppercase() {
                    other.to_ascii_uppercase()
                } else {
                    let mut capital = other.to_owned();
                    capital[..1].make_ascii_uppercase();
                    capital
                };
                return format!(
                    "{}{replace}{}{number}",
                    &body[..at],
                    &body[at + word.len()..]
                );
            }
        }
    }
    name.to_owned()
}
