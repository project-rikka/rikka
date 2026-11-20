use std::f32::consts::PI;

use glam::Vec3;
use rikka_mesh_edit::{EdgeId, ElementKind, VertId};
use rikka_polymesh::{GroupedIndices, Polymesh, acos_approx};

use super::editing::Editing;
use crate::kinds::edge_split::EdgeSplit;
use crate::note::SkipReason;

const ANGLE_EPSILON: f32 = 0.000000175;

pub(crate) fn edge_angles(faces_of: &GroupedIndices, normals: &[Vec3]) -> Vec<f32> {
    faces_of
        .iter()
        .map(|faces| match faces {
            [first, second] => acos_approx(normals[*first as usize].dot(normals[*second as usize])),
            _ => 0.0,
        })
        .collect()
}

pub(crate) fn evaluate(edge_split: &EdgeSplit, mesh: &mut Polymesh) -> Result<(), SkipReason> {
    mesh.ensure_edges();
    let by_angle = edge_split.use_edge_angle && edge_split.split_angle < PI;
    let everything = by_angle && edge_split.split_angle < f32::EPSILON;
    let threshold = (edge_split.split_angle + ANGLE_EPSILON).cos();
    let normals = mesh.face_normals();
    let faces_of = mesh.edge_faces();
    let split: Vec<bool> = (0..mesh.edge_count())
        .map(|edge| {
            let faces = faces_of.get(edge);
            let angle = by_angle
                && faces.len() >= 2
                && (faces.len() > 2
                    || everything
                    || normals[faces[0] as usize].dot(normals[faces[1] as usize]) < threshold);
            let sharp = edge_split.use_edge_sharp && !faces.is_empty() && mesh.is_sharp_edge(edge);
            angle || sharp
        })
        .collect();
    match split.contains(&true) {
        true => split_edges(mesh, &split),
        false => Ok(()),
    }
}

fn split_edges(mesh: &mut Polymesh, split: &[bool]) -> Result<(), SkipReason> {
    let mut editing = Editing::new(mesh)?;
    let edit = &mut editing.mesh;
    let edges: Vec<EdgeId> = edit.edges().collect();
    for &edge in &edges {
        edit.set_edge_tag(edge, split[edge.index()]);
    }
    for vert in edit.verts().collect::<Vec<VertId>>() {
        edit.set_vert_tag(vert, false);
    }
    for &edge in &edges {
        if split[edge.index()] {
            for vert in edit.edge_verts(edge) {
                edit.set_vert_tag(vert, true);
            }
        }
    }
    for &edge in &edges {
        if !split[edge.index()] {
            continue;
        }
        for side in 0..2 {
            let vert = edit.edge_verts(edge)[side];
            if !edit.is_vert_tagged(vert) {
                continue;
            }
            edit.set_vert_tag(vert, false);
            let tagged: Vec<EdgeId> = edit
                .vert_edges(vert)
                .filter(|&around| edit.is_edge_tagged(around))
                .collect();
            edit.separate_vert(vert, &tagged, false)
                .map_err(|_| SkipReason::InvalidMesh)?;
        }
    }
    edit.clear_tags(ElementKind::Vert);
    edit.clear_tags(ElementKind::Edge);
    *mesh = editing.finish();
    Ok(())
}
