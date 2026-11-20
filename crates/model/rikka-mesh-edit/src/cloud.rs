use std::cmp::Ordering;

use glam::Vec3;

use crate::ids::VertId;
use crate::math::{
    angle_signed_on_axis_vectors, newell_cross, normal_tri, normalize, normalize_with_length,
    project_plane_normalized,
};
use crate::mesh::EditMesh;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloudNormal {
    pub normal: Vec3,
    pub center: Vec3,
    pub tangent_index: usize,
}

impl EditMesh {
    #[must_use]
    pub fn verts_normal_from_cloud(&self, verts: &[VertId]) -> CloudNormal {
        let count = verts.len();
        let positions: Vec<Vec3> = verts.iter().map(|&vert| self.position(vert)).collect();
        let inverse = 1.0 / count as f32;
        let mut center = Vec3::ZERO;
        for &position in &positions {
            center += position * inverse;
        }
        let mut a_index = 0;
        let mut distance_max = -1.0f32;
        for (index, &position) in positions.iter().enumerate() {
            let distance = position.distance_squared(center);
            if exceeds(distance, distance_max) {
                a_index = index;
                distance_max = distance;
            }
        }
        let co_a = positions[a_index];
        let dir_a = normalize(co_a - center);
        let mut b_index = None;
        let mut dir_b = Vec3::ZERO;
        let mut distance_max = -1.0f32;
        for (index, &position) in positions.iter().enumerate() {
            if verts[index] == verts[a_index] {
                continue;
            }
            let direction = project_plane_normalized(position - center, dir_a);
            let distance = direction.length_squared();
            if exceeds(distance, distance_max) {
                b_index = Some(index);
                distance_max = distance;
                dir_b = direction;
            }
        }
        let co_b = b_index.map_or(Vec3::ZERO, |index| positions[index]);
        let mut tangent_index = a_index;
        if count <= 3 {
            return CloudNormal {
                normal: normal_tri(center, co_a, co_b),
                center,
                tangent_index,
            };
        }
        let dir_b = normalize(dir_b);
        let mut a_opposite: Option<usize> = None;
        let mut b_opposite: Option<usize> = None;
        let mut dot_a_min = f32::MAX;
        let mut dot_b_min = f32::MAX;
        for (index, &position) in positions.iter().enumerate() {
            if verts[index] != verts[a_index] {
                let dot = dir_a.dot(position);
                if dot < dot_a_min || a_opposite.is_none() {
                    dot_a_min = dot;
                    a_opposite = Some(index);
                }
            }
            if b_index.is_none_or(|b_index| verts[index] != verts[b_index]) {
                let dot = dir_b.dot(position);
                if dot < dot_b_min || b_opposite.is_none() {
                    dot_b_min = dot;
                    b_opposite = Some(index);
                }
            }
        }
        let opposite = |index: Option<usize>| index.map_or(Vec3::ZERO, |index| positions[index]);
        let (mut normal, quad_length) =
            normalize_with_length((co_a - opposite(a_opposite)).cross(co_b - opposite(b_opposite)));
        if quad_length != 0.0 {
            let angles: Vec<f32> = positions
                .iter()
                .map(|&position| {
                    let angle = angle_signed_on_axis_vectors(dir_a, position - center, normal);
                    match angle.is_finite() {
                        true => angle,
                        false => 0.0,
                    }
                })
                .collect();
            let mut order: Vec<usize> = (0..count).collect();
            order.sort_by(|&a, &b| angles[b].total_cmp(&angles[a]));
            let mut refine = Vec3::ZERO;
            let mut previous = positions[order[count - 1]];
            for &index in &order {
                let current = positions[index];
                refine += newell_cross(previous, current);
                previous = current;
            }
            let (refine, length) = normalize_with_length(refine);
            if length != 0.0 {
                let mut distance_max = -1.0f32;
                for (index, &position) in positions.iter().enumerate() {
                    let direction = project_plane_normalized(position - center, refine);
                    let distance = direction.length_squared();
                    if exceeds(distance, distance_max) {
                        tangent_index = index;
                        distance_max = distance;
                    }
                }
                normal = refine;
            }
        }
        CloudNormal {
            normal,
            center,
            tangent_index,
        }
    }
}

fn exceeds(value: f32, limit: f32) -> bool {
    !matches!(
        value.partial_cmp(&limit),
        Some(Ordering::Less | Ordering::Equal)
    )
}
