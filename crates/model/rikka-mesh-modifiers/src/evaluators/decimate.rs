use rikka_mesh_reduce::{
    Axis, Collapse, Delimit as ReduceDelimit, decimate_collapse, decimate_planar,
    decimate_unsubdivide,
};
use rikka_polymesh::Polymesh;

use super::Scope;
use super::editing::Editing;
use super::vertex_group::Group;
use crate::kinds::decimate::{Decimate, DecimateType, Delimit, SymmetryAxis};
use crate::note::{NoteKind, SkipReason};

const MIN_FACES: usize = 4;

pub(crate) fn evaluate(
    settings: &Decimate,
    mesh: &mut Polymesh,
    scope: &mut Scope<'_>,
) -> Result<(), SkipReason> {
    let unchanged = match settings.decimate_type {
        DecimateType::Collapse => settings.ratio == 1.0,
        DecimateType::Unsubdivide => settings.iterations == 0,
        DecimateType::Planar => settings.angle_limit == 0.0,
    };
    if unchanged {
        scope.note(NoteKind::FaceCount(mesh.face_count() as u32));
        return Ok(());
    }
    if mesh.face_count() < MIN_FACES {
        scope.note(NoteKind::FaceCount(mesh.face_count() as u32));
        return Err(SkipReason::TooFewFaces);
    }
    let weights = match settings.decimate_type {
        DecimateType::Collapse => vertex_weights(settings, mesh),
        DecimateType::Unsubdivide | DecimateType::Planar => None,
    };
    let mut editing = Editing::new(mesh)?;
    match settings.decimate_type {
        DecimateType::Collapse => decimate_collapse(
            &mut editing.mesh,
            &Collapse {
                factor: settings.ratio,
                weights,
                weight_factor: settings.vertex_group_factor,
                triangulate: settings.use_collapse_triangulate,
                symmetry: settings
                    .use_symmetry
                    .then_some(match settings.symmetry_axis {
                        SymmetryAxis::X => Axis::X,
                        SymmetryAxis::Y => Axis::Y,
                        SymmetryAxis::Z => Axis::Z,
                    }),
                ..Collapse::default()
            },
        ),
        DecimateType::Unsubdivide => {
            decimate_unsubdivide(&mut editing.mesh, u32::from(settings.iterations));
        }
        DecimateType::Planar => decimate_planar(
            &mut editing.mesh,
            settings.angle_limit,
            settings.use_dissolve_boundaries,
            delimit(settings.delimit),
        ),
    }
    *mesh = editing.finish();
    scope.note(NoteKind::FaceCount(mesh.face_count() as u32));
    Ok(())
}

fn vertex_weights(settings: &Decimate, mesh: &Polymesh) -> Option<Vec<f32>> {
    if settings.vertex_group_factor <= 0.0 {
        return None;
    }
    let Group::Weighted(group) = Group::find(mesh, &settings.vertex_group) else {
        return None;
    };
    Some(
        (0..mesh.point_count())
            .map(|point| {
                let weight = mesh.vertex_groups.weight(point, group);
                match settings.invert_vertex_group {
                    true => 1.0 - weight,
                    false => weight,
                }
            })
            .collect(),
    )
}

fn delimit(delimit: Delimit) -> ReduceDelimit {
    [
        (Delimit::NORMAL, ReduceDelimit::NORMAL),
        (Delimit::MATERIAL, ReduceDelimit::MATERIAL),
        (Delimit::SEAM, ReduceDelimit::SEAM),
        (Delimit::SHARP, ReduceDelimit::SHARP),
        (Delimit::UV, ReduceDelimit::UV),
    ]
    .into_iter()
    .filter(|(flag, _)| delimit.contains(*flag))
    .fold(ReduceDelimit::empty(), |all, (_, flag)| all | flag)
}
