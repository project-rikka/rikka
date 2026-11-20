use std::collections::VecDeque;

use glam::Vec3;
use rikka_mesh_edit::{
    EdgeId, EditMesh, Element, IdMap, IdSet, LoopId, MathLayer, SelectMode, VertId,
};

use crate::common::{Counts, Removed, selected_edges, selected_verts};
use crate::error::{Error, Result};
use crate::layers::{self, Bounds};
use crate::weld::WeldOptions;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MergeType {
    #[default]
    Center,
    Cursor,
    Collapse,
    First,
    Last,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MergeOptions {
    pub kind: MergeType,
    pub uvs: bool,
    pub cursor: Vec3,
}

pub fn merge(mesh: &mut EditMesh, options: &MergeOptions) -> Result<Removed> {
    let counts = Counts::new(mesh);
    let verts = selected_verts(mesh);
    if verts.is_empty() {
        return Ok(counts.removed(mesh));
    }
    match options.kind {
        MergeType::Center => {
            let mut center = Vec3::ZERO;
            for &vert in &verts {
                center += mesh.position(vert);
            }
            center *= 1.0 / verts.len() as f32;
            merge_at(mesh, &verts, center, options.uvs);
        }
        MergeType::Cursor => merge_at(mesh, &verts, options.cursor, options.uvs),
        MergeType::First | MergeType::Last => {
            let history = mesh.select_history();
            let element = match options.kind {
                MergeType::First => history.first(),
                _ => history.last(),
            };
            let Some(&Element::Vert(target)) = element else {
                return Err(Error::NoHistoryVertex);
            };
            if !mesh.is_vert_selected(target) {
                return Err(Error::NoHistoryVertex);
            }
            if options.uvs {
                merge_face_data_at(mesh, &verts, target);
            }
            point_merge(mesh, &verts, None, Some(target));
        }
        MergeType::Collapse => {
            let edges = selected_edges(mesh);
            collapse_geometry(mesh, &edges, options.uvs);
        }
    }
    if !mesh.select_mode().contains(SelectMode::VERT) {
        mesh.deselect_all();
    }
    mesh.update_normals();
    Ok(counts.removed(mesh))
}

pub fn collapse_edges(mesh: &mut EditMesh) -> Result<Removed> {
    let counts = Counts::new(mesh);
    let edges = selected_edges(mesh);
    if edges.is_empty() {
        return Ok(counts.removed(mesh));
    }
    collapse_geometry(mesh, &edges, true);
    mesh.update_normals();
    Ok(counts.removed(mesh))
}

fn merge_at(mesh: &mut EditMesh, verts: &[VertId], position: Vec3, uvs: bool) {
    if uvs {
        average_face_data(mesh, verts);
    }
    point_merge(mesh, verts, Some(position), None);
}

pub fn point_merge(
    mesh: &mut EditMesh,
    verts: &[VertId],
    position: Option<Vec3>,
    target: Option<VertId>,
) {
    let mut targets: IdMap<VertId, VertId> = IdMap::new();
    let mut chosen = target.filter(|&vert| mesh.contains_vert(vert));
    let explicit = chosen.is_some();
    for &vert in verts {
        if !mesh.contains_vert(vert) {
            continue;
        }
        match chosen {
            None => {
                chosen = Some(vert);
                if let Some(position) = position {
                    mesh.set_position(vert, position);
                }
            }
            Some(chosen) if chosen != vert => {
                targets.insert(vert, chosen);
            }
            Some(_) => {}
        }
    }
    mesh.weld_verts(
        &targets,
        &WeldOptions {
            use_centroid: false,
            average_vert_data: !explicit,
        },
    );
}

pub fn merge_face_data_at(mesh: &mut EditMesh, verts: &[VertId], target: VertId) {
    if !mesh.contains_vert(target) {
        return;
    }
    let face_count = mesh.vert_face_count(target);
    if face_count == 0 {
        return;
    }
    let factor = 1.0 / face_count as f32;
    let layers = mesh.loop_math_layers();
    let loops = mesh.vert_loops(target).collect::<Vec<_>>();
    let first = loops[0];
    for &lp in &loops {
        for layer in &layers {
            layers::scale(mesh, layer, lp, factor);
            if lp != first {
                layers::add(mesh, layer, first, lp);
            }
        }
    }
    for &vert in verts {
        if !mesh.contains_vert(vert) {
            continue;
        }
        let around = mesh.vert_loops(vert).collect::<Vec<_>>();
        for lp in around {
            if lp != first {
                mesh.copy_loop_data(first, lp);
            }
        }
    }
}

pub fn average_face_data(mesh: &mut EditMesh, verts: &[VertId]) {
    let loops: Vec<LoopId> = verts
        .iter()
        .filter(|&&vert| mesh.contains_vert(vert))
        .flat_map(|&vert| mesh.vert_loops(vert).collect::<Vec<_>>())
        .collect();
    for layer in mesh.loop_math_layers() {
        let mut bounds = Bounds::new(layer.kind);
        for &lp in &loops {
            if let Some(value) = layers::read(mesh, &layer, lp) {
                bounds.include(value);
            }
        }
        let center = bounds.center();
        for &lp in &loops {
            layers::write(mesh, &layer, lp, center);
        }
    }
}

pub fn collapse_geometry(mesh: &mut EditMesh, edges: &[EdgeId], uvs: bool) {
    let marked: IdSet<EdgeId> = edges
        .iter()
        .copied()
        .filter(|&edge| mesh.contains_edge(edge))
        .collect();
    if marked.is_empty() {
        return;
    }
    if uvs {
        collapse_face_data(mesh, &marked);
    }
    let mut targets: IdMap<VertId, VertId> = IdMap::new();
    let mut visited: IdSet<EdgeId> = IdSet::new();
    let mut tagged: IdSet<VertId> = IdSet::new();
    let mut queue: VecDeque<EdgeId> = VecDeque::new();
    let mut walked: Vec<EdgeId> = Vec::new();
    let all: Vec<EdgeId> = mesh.edges().collect();
    for start in all {
        if !marked.contains(start) {
            continue;
        }
        let begin = mesh.edge_verts(start)[0];
        for edge in mesh.vert_edges(begin) {
            if marked.contains(edge) && visited.insert(edge) {
                queue.push_back(edge);
            }
        }
        let mut center = Vec3::ZERO;
        let mut count = 0usize;
        while let Some(edge) = queue.pop_front() {
            let [a, b] = mesh.edge_verts(edge);
            for vert in [a, b] {
                for next in mesh.vert_edges(vert) {
                    if marked.contains(next) && visited.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
            walked.push(edge);
            center += mesh.position(a);
            center += mesh.position(b);
            count += 2;
            tagged.remove(a);
            tagged.remove(b);
        }
        let Some(&last) = walked.last() else {
            continue;
        };
        center *= 1.0 / count as f32;
        let target = mesh.edge_verts(last)[0];
        while let Some(edge) = walked.pop() {
            for vert in mesh.edge_verts(edge) {
                mesh.set_position(vert, center);
                if vert != target && tagged.insert(vert) {
                    targets.insert(vert, target);
                }
            }
        }
    }
    mesh.weld_verts(&targets, &WeldOptions::default());
}

fn collapse_face_data(mesh: &mut EditMesh, marked: &IdSet<EdgeId>) {
    let layers = mesh.loop_math_layers();
    if layers.is_empty() {
        return;
    }
    for layer in &layers {
        collapse_layer(mesh, marked, layer);
    }
}

fn collapse_layer(mesh: &mut EditMesh, marked: &IdSet<EdgeId>, layer: &MathLayer) {
    let mut visited: IdSet<LoopId> = IdSet::new();
    let mut stack: Vec<LoopId> = Vec::new();
    let mut island: Vec<LoopId> = Vec::new();
    let faces: Vec<_> = mesh.faces().collect();
    for face in faces {
        for start in mesh.face_loops_vec(face) {
            if !marked.contains(mesh.loop_edge(start)) || !visited.insert(start) {
                continue;
            }
            stack.push(start);
            while let Some(lp) = stack.pop() {
                island.push(lp);
                if !marked.contains(mesh.loop_edge(lp)) {
                    continue;
                }
                for pivot in [lp, mesh.loop_next(lp)] {
                    let pivot_vert = mesh.loop_vert(pivot);
                    let around: Vec<LoopId> = mesh.vert_loops(pivot_vert).collect();
                    for first in around {
                        let mut radial = first;
                        loop {
                            let usable = !visited.contains(radial)
                                && (mesh.loop_vert(radial) == pivot_vert
                                    || marked.contains(mesh.loop_edge(radial)));
                            if usable {
                                let other = match mesh.loop_vert(radial) == pivot_vert {
                                    true => radial,
                                    false => mesh.loop_next(radial),
                                };
                                if layer.loops_equal(mesh, pivot, other) {
                                    visited.insert(radial);
                                    stack.push(radial);
                                }
                            }
                            radial = mesh.loop_radial_next(radial);
                            if radial == first {
                                break;
                            }
                        }
                        let boundary = mesh.loop_prev(first);
                        let edge = mesh.loop_edge(boundary);
                        if mesh.is_edge_boundary(edge)
                            && marked.contains(edge)
                            && visited.insert(boundary)
                        {
                            stack.push(boundary);
                        }
                    }
                }
            }
            let mut bounds = Bounds::new(layer.kind);
            for &lp in &island {
                if let Some(value) = layers::read(mesh, layer, lp) {
                    bounds.include(value);
                }
            }
            let center = bounds.center();
            for lp in island.drain(..) {
                layers::write(mesh, layer, lp, center);
            }
        }
    }
}
