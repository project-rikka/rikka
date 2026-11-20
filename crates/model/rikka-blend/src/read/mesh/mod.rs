pub(crate) mod attributes;
pub(crate) mod source;

use rikka_model_core::glam::Vec3;
use rikka_polymesh::{Polymesh, Shading};

use super::modifier::{self, Deform, Notes, Scope};
use crate::file::Record;
use source::Custom;

pub(crate) struct MeshEvaluation<'a> {
    pub(crate) mesh: Polymesh,
    pub(crate) shading: Shading,
    pub(crate) deform: Option<Deform<'a>>,
    pub(crate) pinned_key: Option<usize>,
}

pub(crate) fn evaluate<'a>(
    object: Record<'a>,
    data: Record<'a>,
    scope: &Scope<'_, 'a>,
    notes: &mut Notes,
) -> MeshEvaluation<'a> {
    let name = object.id_name();
    let source = source::read(data, Some(object));
    if source.absolute_keys {
        notes.unsupported(format!(
            "object \"{name}\": absolute shape keys read at their basis"
        ));
    }
    if source.repaired {
        notes.repaired(format!(
            "object \"{name}\": dropped faces or edges that name missing vertices or corners"
        ));
    }
    let mut mesh = source.mesh;
    fold_shape_key_groups(&mut mesh);
    if let Some(custom) = &source.custom {
        if let Some(angle) = source.auto_smooth {
            mesh.sharpen_by_angle(angle, true);
        }
        let normals = object_space_normals(&mesh, custom);
        if normals.len() == mesh.corner_count() {
            mesh.custom_normals_mut().copy_from_slice(&normals);
        }
    }
    let deform = modifier::apply(object, &mut mesh, scope, notes);
    if source.custom.is_none()
        && let Some(angle) = source.auto_smooth
    {
        mesh.sharpen_by_angle(angle, true);
    }
    let shading = match source.legacy_normals {
        true => Shading::Vertex,
        false => Shading::Split,
    };
    MeshEvaluation {
        mesh,
        shading,
        deform,
        pinned_key: source.pinned_key,
    }
}

pub(crate) fn object_space_normals(mesh: &Polymesh, custom: &Custom) -> Vec<Vec3> {
    match custom {
        Custom::Encoded(encoded) => mesh.decode_custom_normals(encoded),
        Custom::Corner(normals) => normals
            .iter()
            .map(|normal| normal.normalize_or_zero())
            .collect(),
        Custom::Point(normals) => mesh
            .corner_verts
            .iter()
            .map(|&point| {
                normals
                    .get(point as usize)
                    .map_or(Vec3::ZERO, |normal| normal.normalize_or_zero())
            })
            .collect(),
        Custom::Face(normals) => (0..mesh.face_count())
            .flat_map(|face| {
                let normal = normals
                    .get(face)
                    .map_or(Vec3::ZERO, |normal| normal.normalize_or_zero());
                std::iter::repeat_n(normal, mesh.face_size(face))
            })
            .collect(),
    }
}

fn fold_shape_key_groups(mesh: &mut Polymesh) {
    let originals: Vec<Vec<Vec3>> = mesh
        .shape_keys
        .keys
        .iter()
        .map(|key| key.positions.clone())
        .collect();
    let groups = &mesh.vertex_groups;
    for key in &mut mesh.shape_keys.keys {
        let Some(name) = key.vertex_group.take() else {
            continue;
        };
        let Some(group) = groups.index(&name).filter(|_| groups.has_weights()) else {
            continue;
        };
        let reference = &originals[key.relative_to as usize];
        for (point, position) in key.positions.iter_mut().enumerate() {
            let weight = groups.weight(point, group);
            *position = reference[point] + (*position - reference[point]) * weight;
        }
    }
}
