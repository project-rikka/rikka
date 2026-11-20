use glam::Vec3;

use crate::error::Result;
use crate::ids::FaceId;
use crate::math::normalize_with_length;
use crate::mesh::EditMesh;
use crate::sets::IdSet;

impl EditMesh {
    pub fn recalc_face_normals(&mut self, faces: &IdSet<FaceId>) -> Result<Vec<FaceId>> {
        recalc_face_normals(self, faces)
    }
}

fn recalc_face_normals(mesh: &mut EditMesh, faces: &IdSet<FaceId>) -> Result<Vec<FaceId>> {
    let groups = face_groups_manifold(mesh);
    let mut flip = vec![false; mesh.face_capacity()];
    let mut visited = vec![false; mesh.face_capacity()];
    let mut stack: Vec<FaceId> = Vec::new();
    for group in &groups {
        if !group.iter().any(|&face| faces.contains(face)) {
            continue;
        }
        let (start, start_flipped) = outermost_face(mesh, group);
        flip[group[start].index()] = start_flipped;
        stack.push(group[start]);
        visited[group[start].index()] = true;
        while let Some(face) = stack.pop() {
            let flip_state = flip[face.index()];
            for lp in mesh.face_loops(face) {
                let other = mesh.loop_radial_next(lp);
                if other == lp || !mesh.is_edge_manifold(mesh.loop_edge(lp)) {
                    continue;
                }
                let other_face = mesh.loop_face(other);
                if visited[other_face.index()] {
                    continue;
                }
                visited[other_face.index()] = true;
                flip[other_face.index()] =
                    (mesh.loop_vert(other) == mesh.loop_vert(lp)) != flip_state;
                stack.push(other_face);
            }
        }
    }
    let mut flipped = Vec::new();
    for group in &groups {
        for &face in group {
            if faces.contains(face) && flip[face.index()] {
                mesh.reverse_face(face)?;
                flipped.push(face);
            }
        }
    }
    flipped.sort_unstable_by_key(|face| face.index());
    Ok(flipped)
}

fn face_groups_manifold(mesh: &EditMesh) -> Vec<Vec<FaceId>> {
    let mut tagged = vec![false; mesh.face_capacity()];
    let mut groups = Vec::new();
    let mut stack: Vec<FaceId> = Vec::new();
    for seed in mesh.faces() {
        if tagged[seed.index()] {
            continue;
        }
        tagged[seed.index()] = true;
        stack.push(seed);
        let mut group = Vec::new();
        while let Some(face) = stack.pop() {
            group.push(face);
            for lp in mesh.face_loops(face) {
                let mut radial = mesh.loop_radial_next(lp);
                if radial == lp || !mesh.is_edge_manifold(mesh.loop_edge(lp)) {
                    continue;
                }
                while radial != lp {
                    let other = mesh.loop_face(radial);
                    if !tagged[other.index()] {
                        tagged[other.index()] = true;
                        stack.push(other);
                    }
                    radial = mesh.loop_radial_next(radial);
                }
            }
        }
        groups.push(group);
    }
    groups
}

fn normalize_length(vector: &mut Vec3) -> f32 {
    let (normalized, length) = normalize_with_length(*vector);
    *vector = normalized;
    length
}

fn outermost_face(mesh: &EditMesh, faces: &[FaceId]) -> (usize, bool) {
    let epsilon = f32::EPSILON;
    let mut center = Vec3::ZERO;
    let mut area_sum = 0.0f32;
    let center_factor = 1.0 / faces.len() as f32;
    for &face in faces {
        let area = mesh.face_area(face);
        let face_center = mesh.face_center_median_weighted(face);
        center += face_center * (center_factor * area);
        area_sum += area;
    }
    if area_sum != 0.0 {
        center *= 1.0 / area_sum;
    }
    let mut best_distance = epsilon;
    let mut best_edge_dot = -f32::MAX;
    let mut best_loop_dot = -f32::MAX;
    let mut start = 0;
    let mut is_flip = false;
    for (index, &face) in faces.iter().enumerate() {
        for lp in mesh.face_loops(face) {
            let here = mesh.position(mesh.loop_vert(lp));
            let mut direction = here - center;
            let distance = direction.length_squared();
            let best_by_distance = distance > best_distance;
            if !(best_by_distance || distance == best_distance) {
                continue;
            }
            direction *= 1.0 / distance.sqrt();
            let mut towards_next = mesh.position(mesh.loop_next_vert(lp)) - here;
            let mut towards_previous = mesh.position(mesh.loop_vert(mesh.loop_prev(lp))) - here;
            if normalize_length(&mut towards_next) <= epsilon
                || normalize_length(&mut towards_previous) <= epsilon
            {
                continue;
            }
            let edge_dot = direction
                .dot(towards_next)
                .max(direction.dot(towards_previous));
            let best_by_edge = edge_dot > best_edge_dot;
            if !(best_by_distance || best_by_edge || edge_dot == best_edge_dot) {
                continue;
            }
            let mut loop_direction = towards_next.cross(towards_previous);
            if normalize_length(&mut loop_direction) <= epsilon {
                continue;
            }
            if loop_direction.dot(mesh.face_normal(face)) < 0.0 {
                loop_direction = -loop_direction;
            }
            let loop_dot_signed = direction.dot(loop_direction);
            let loop_dot = loop_dot_signed.abs();
            if best_by_distance || best_by_edge || loop_dot > best_loop_dot {
                best_distance = distance;
                best_edge_dot = edge_dot;
                best_loop_dot = loop_dot;
                start = index;
                is_flip = loop_dot_signed < 0.0;
            }
        }
    }
    (start, is_flip)
}
