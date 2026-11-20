use glam::{Mat3, Vec3};
use rikka_polymesh::Polymesh;

use super::vertex_group::Group;
use crate::kinds::corrective_smooth::{CorrectiveSmooth, RestSource, SmoothType};
use crate::math::normalize;
use crate::note::SkipReason;

const PARALLEL_EPSILON: f32 = f32::EPSILON * 10.0;

pub(crate) fn evaluate(
    settings: &CorrectiveSmooth,
    mesh: &mut Polymesh,
    original: &[Vec3],
) -> Result<(), SkipReason> {
    mesh.ensure_edges();
    let weights = smooth_weights(settings, mesh);
    let mut positions = std::mem::take(&mut mesh.positions);
    if settings.use_only_smooth {
        smooth(settings, mesh, &mut positions, weights.as_deref());
        mesh.positions = positions;
        return Ok(());
    }
    let rest = match settings.rest_source {
        RestSource::Bind if !settings.is_bound() => {
            mesh.positions = positions;
            return Err(SkipReason::BindRequired);
        }
        RestSource::Bind => &settings.bind_coords[..],
        RestSource::Original => original,
    };
    if rest.len() != positions.len() {
        mesh.positions = positions;
        return Err(SkipReason::VertexCountMismatch);
    }
    let deltas = rest_deltas(settings, mesh, rest, weights.as_deref());
    smooth(settings, mesh, &mut positions, weights.as_deref());
    let tangents = tangent_spaces(mesh, &positions);
    let mut per_point = vec![0.0f32; positions.len()];
    for (corner, tangent) in tangents.iter().enumerate() {
        if let Some((_, weight)) = tangent {
            per_point[mesh.corner_verts[corner] as usize] += weight;
        }
    }
    for (corner, tangent) in tangents.iter().enumerate() {
        let point = mesh.corner_verts[corner] as usize;
        let Some((space, weight)) = tangent else {
            continue;
        };
        let weight = weight / per_point[point];
        if weight.is_nan() || weight <= 0.0 {
            continue;
        }
        let delta = (*space * deltas[corner]) * weight;
        positions[point] += delta * settings.scale;
    }
    mesh.positions = positions;
    Ok(())
}

fn smooth_weights(settings: &CorrectiveSmooth, mesh: &Polymesh) -> Option<Vec<f32>> {
    let group = Group::find(mesh, &settings.vertex_group);
    let weighted = matches!(group, Group::Weighted(_));
    if !weighted && !settings.use_pin_boundary {
        return None;
    }
    let mut weights: Vec<f32> = (0..mesh.point_count())
        .map(|point| match weighted {
            true => group.weight(mesh, point, settings.invert_vertex_group),
            false => 1.0,
        })
        .collect();
    if settings.use_pin_boundary {
        let mut uses = vec![0u8; mesh.edge_count()];
        for &edge in &mesh.corner_edges {
            let value = &mut uses[edge as usize];
            *value |= value.wrapping_add(1);
        }
        for (edge, &count) in uses.iter().enumerate() {
            if count == 1 {
                let [a, b] = mesh.edges[edge];
                weights[a as usize] = 0.0;
                weights[b as usize] = 0.0;
            }
        }
    }
    Some(weights)
}

fn smooth(
    settings: &CorrectiveSmooth,
    mesh: &Polymesh,
    positions: &mut [Vec3],
    weights: Option<&[f32]>,
) {
    match settings.smooth_type {
        SmoothType::Simple => smooth_simple(settings, mesh, positions, weights),
        SmoothType::LengthWeighted => smooth_length_weighted(settings, mesh, positions, weights),
    }
}

fn edge_counts(mesh: &Polymesh, points: usize) -> Vec<f32> {
    let mut counts = vec![0.0f32; points];
    for &[a, b] in &mesh.edges {
        counts[a as usize] += 1.0;
        counts[b as usize] += 1.0;
    }
    counts
}

fn smooth_simple(
    settings: &CorrectiveSmooth,
    mesh: &Polymesh,
    positions: &mut [Vec3],
    weights: Option<&[f32]>,
) {
    let lambda = settings.factor;
    let factors: Vec<f32> = edge_counts(mesh, positions.len())
        .iter()
        .enumerate()
        .map(|(point, &count)| {
            let inverse = if count != 0.0 { 1.0 / count } else { 1.0 };
            match weights {
                Some(weights) => weights[point] * lambda * inverse,
                None => lambda * inverse,
            }
        })
        .collect();
    let mut deltas = vec![Vec3::ZERO; positions.len()];
    for _ in 0..settings.iterations {
        for &[a, b] in &mesh.edges {
            let direction = positions[b as usize] - positions[a as usize];
            deltas[a as usize] += direction;
            deltas[b as usize] -= direction;
        }
        for ((position, delta), factor) in positions.iter_mut().zip(&mut deltas).zip(&factors) {
            *position += *delta * *factor;
            *delta = Vec3::ZERO;
        }
    }
}

fn smooth_length_weighted(
    settings: &CorrectiveSmooth,
    mesh: &Polymesh,
    positions: &mut [Vec3],
    weights: Option<&[f32]>,
) {
    let lambda = settings.factor * 2.0;
    let counts = edge_counts(mesh, positions.len());
    let mut deltas = vec![Vec3::ZERO; positions.len()];
    let mut lengths = vec![0.0f32; positions.len()];
    for _ in 0..settings.iterations {
        for &[a, b] in &mesh.edges {
            let direction = positions[b as usize] - positions[a as usize];
            let distance = direction.length();
            let direction = direction * distance;
            deltas[a as usize] += direction;
            deltas[b as usize] -= direction;
            lengths[a as usize] += distance;
            lengths[b as usize] += distance;
        }
        for point in 0..positions.len() {
            let divisor = lengths[point] * counts[point];
            if divisor > PARALLEL_EPSILON {
                let factor = match weights {
                    Some(weights) => lambda * weights[point],
                    None => lambda,
                };
                positions[point] += deltas[point] * (factor / divisor);
            }
            deltas[point] = Vec3::ZERO;
            lengths[point] = 0.0;
        }
    }
}

fn tangent_spaces(mesh: &Polymesh, positions: &[Vec3]) -> Vec<Option<(Mat3, f32)>> {
    let mut spaces = vec![None; mesh.corner_count()];
    for face in 0..mesh.face_count() {
        let range = mesh.face(face);
        let at = |corner: usize| positions[mesh.corner_verts[corner] as usize];
        let mut previous_direction = normalize(at(range.end - 2) - at(range.end - 1));
        let mut current = range.end - 1;
        for next in range.clone() {
            let next_direction = normalize(at(current) - at(next));
            spaces[current] = tangent_loop(previous_direction, next_direction).map(|space| {
                let weight = safe_acos_approx(next_direction.dot(previous_direction)).abs();
                (space, weight)
            });
            previous_direction = next_direction;
            current = next;
        }
    }
    spaces
}

fn tangent_loop(previous: Vec3, next: Vec3) -> Option<Mat3> {
    if (previous - next).abs().max_element() <= PARALLEL_EPSILON {
        return None;
    }
    let normal = normalize(previous.cross(next));
    let tangent = normalize(previous + next);
    let bitangent = normal.cross(tangent);
    Some(Mat3::from_cols(tangent, bitangent, normal))
}

fn safe_acos_approx(x: f32) -> f32 {
    rikka_polymesh::acos_approx(x.clamp(-1.0, 1.0))
}

fn rest_deltas(
    settings: &CorrectiveSmooth,
    mesh: &Polymesh,
    rest: &[Vec3],
    weights: Option<&[f32]>,
) -> Vec<Vec3> {
    let mut smoothed = rest.to_vec();
    smooth(settings, mesh, &mut smoothed, weights);
    let spaces = tangent_spaces(mesh, &smoothed);
    mesh.corner_verts
        .iter()
        .zip(&spaces)
        .map(|(&point, space)| {
            let point = point as usize;
            let delta = rest[point] - smoothed[point];
            let space = space.map_or(Mat3::IDENTITY, |(space, _)| space);
            invert(space) * delta
        })
        .collect()
}

fn invert(m: Mat3) -> Mat3 {
    let c = [m.x_axis, m.y_axis, m.z_axis];
    let (m00, m01, m02) = (c[0].x, c[0].y, c[0].z);
    let (m10, m11, m12) = (c[1].x, c[1].y, c[1].z);
    let (m20, m21, m22) = (c[2].x, c[2].y, c[2].z);
    let adjoint = [
        Vec3::new(
            m11 * m22 - m12 * m21,
            -m01 * m22 + m02 * m21,
            m01 * m12 - m02 * m11,
        ),
        Vec3::new(
            -m10 * m22 + m12 * m20,
            m00 * m22 - m02 * m20,
            -m00 * m12 + m02 * m10,
        ),
        Vec3::new(
            m10 * m21 - m11 * m20,
            -m00 * m21 + m01 * m20,
            m00 * m11 - m01 * m10,
        ),
    ];
    let determinant = m00 * (m11 * m22 - m12 * m21) - m10 * (m01 * m22 - m02 * m21)
        + m20 * (m01 * m12 - m02 * m11);
    if determinant == 0.0 {
        return m.transpose();
    }
    let scale = 1.0 / determinant;
    Mat3::from_cols(adjoint[0] * scale, adjoint[1] * scale, adjoint[2] * scale)
}
