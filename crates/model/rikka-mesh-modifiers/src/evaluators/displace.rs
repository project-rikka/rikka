use glam::{Mat3, Vec3};
use rikka_polymesh::Polymesh;

use super::Scope;
use super::vertex_group::Group;
use crate::PARALLEL;
use crate::kinds::displace::{Displace, DisplaceDirection, DisplaceSpace, TextureCoordinates};
use crate::normals::shading_point_normals;
use crate::note::SkipReason;
use crate::space::inverse;

const DELTA_LIMIT: f32 = 10000.0;
const GRAIN: usize = 1 << 13;

pub(crate) fn evaluate(
    displace: &Displace,
    mesh: &mut Polymesh,
    scope: &Scope<'_>,
) -> Result<(), SkipReason> {
    let ctx = scope.ctx;
    let group = Group::find(mesh, &displace.vertex_group);
    if group == Group::Unweighted {
        return Ok(());
    }
    let texture = match displace.texture {
        Some(texture) => Some(ctx.texture(texture).ok_or(SkipReason::MissingTexture)?),
        None => None,
    };
    let coordinates = texture.map(|_| texture_coordinates(displace, mesh, scope));
    let global = displace.space == DisplaceSpace::Global;
    let transposed = Mat3::from_mat4(ctx.object_matrix()).transpose();
    let normals = match displace.direction {
        DisplaceDirection::Normal => Some(mesh.point_normals()),
        DisplaceDirection::CustomNormal => Some(shading_point_normals(mesh)),
        _ => None,
    };
    let fixed = 1.0 - displace.mid_level;
    let mut positions = std::mem::take(&mut mesh.positions);
    let source = &*mesh;
    PARALLEL
        .grain(GRAIN)
        .fill(&mut positions, |point, position| {
            let mut strength = displace.strength;
            if let Group::Weighted(_) = group {
                let weight = group.weight(source, point, displace.invert_vertex_group);
                if weight == 0.0 {
                    return;
                }
                strength *= weight;
            }
            let sample = match (texture, &coordinates) {
                (Some(texture), Some(coordinates)) => Some(texture.sample(coordinates[point])),
                _ => None,
            };
            let delta = (sample.map_or(fixed, |sample| sample.intensity - displace.mid_level)
                * strength)
                .clamp(-DELTA_LIMIT, DELTA_LIMIT);
            let axis = |index: usize| match global {
                true => transposed.col(index),
                false => Vec3::AXES[index],
            };
            let shift = match displace.direction {
                DisplaceDirection::X => axis(0) * delta,
                DisplaceDirection::Y => axis(1) * delta,
                DisplaceDirection::Z => axis(2) * delta,
                DisplaceDirection::RgbToXyz => {
                    let color = sample.map_or(Vec3::ZERO, |sample| sample.color.truncate());
                    let local = color - Vec3::splat(displace.mid_level);
                    let local = match global {
                        true => transposed * local,
                        false => local,
                    };
                    local * strength
                }
                DisplaceDirection::Normal | DisplaceDirection::CustomNormal => {
                    normals
                        .as_ref()
                        .map_or(Vec3::ZERO, |normals| normals[point])
                        * delta
                }
            };
            *position += shift;
        });
    mesh.positions = positions;
    Ok(())
}

fn texture_coordinates(displace: &Displace, mesh: &Polymesh, scope: &Scope<'_>) -> Vec<Vec3> {
    let ctx = scope.ctx;
    let world = ctx.object_matrix();
    let mut mapping = displace.texture_coords;
    let mut reference = None;
    if mapping == TextureCoordinates::Object {
        match displace
            .texture_coords_object
            .and_then(|object| ctx.matrix(object).map(|matrix| (object, matrix)))
        {
            Some((object, matrix)) => {
                let placed = match displace.texture_coords_bone.is_empty() {
                    true => matrix,
                    false => ctx
                        .bone_matrix(object, &displace.texture_coords_bone)
                        .map_or(matrix, |bone| matrix * bone),
                };
                reference = Some(inverse(placed));
            }
            None => mapping = TextureCoordinates::Local,
        }
    }
    if mapping == TextureCoordinates::Uv {
        let uv_map = match mesh.uv_maps.contains(&displace.uv_layer) {
            true => mesh.uv_map(&displace.uv_layer),
            false => mesh.active_uv_map(),
        };
        if let Some(uvs) = uv_map {
            let mut coordinates = vec![Vec3::ZERO; mesh.point_count()];
            let mut done = vec![false; mesh.point_count()];
            for (corner, &point) in mesh.corner_verts.iter().enumerate() {
                let point = point as usize;
                if !done[point] {
                    let uv = uvs[corner] * 2.0 - 1.0;
                    coordinates[point] = Vec3::new(uv.x, uv.y, 0.0);
                    done[point] = true;
                }
            }
            return coordinates;
        }
        mapping = TextureCoordinates::Local;
    }
    mesh.positions
        .iter()
        .map(|&position| match mapping {
            TextureCoordinates::Global => world.transform_point3(position),
            TextureCoordinates::Object => reference
                .unwrap_or(glam::Mat4::IDENTITY)
                .transform_point3(world.transform_point3(position)),
            TextureCoordinates::Local | TextureCoordinates::Uv => position,
        })
        .collect()
}
