use rikka_mesh_edit::{EdgeId, EditMesh, IdSet, LoopId};

use crate::common::{Counts, Removed, selected_edges};
use crate::error::{Error, Result};
use crate::math::normalize_with_length;
use crate::merge::collapse_geometry;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DissolveDegenerateOptions {
    pub threshold: f32,
}

impl Default for DissolveDegenerateOptions {
    fn default() -> DissolveDegenerateOptions {
        DissolveDegenerateOptions { threshold: 1.0e-4 }
    }
}

pub fn dissolve_degenerate(
    mesh: &mut EditMesh,
    options: &DissolveDegenerateOptions,
) -> Result<Removed> {
    if !(options.threshold >= 0.0 && options.threshold.is_finite()) {
        return Err(Error::OutOfRange("threshold"));
    }
    let counts = Counts::new(mesh);
    let edges = selected_edges(mesh);
    dissolve_degenerate_geometry(mesh, &edges, options.threshold);
    if counts.changed(mesh) {
        mesh.select_flush_from_verts(true);
        mesh.update_normals();
    }
    Ok(counts.removed(mesh))
}

pub fn dissolve_degenerate_geometry(mesh: &mut EditMesh, edges: &[EdgeId], distance: f32) {
    let distance_squared = distance * distance;
    let mut marked: IdSet<EdgeId> = edges
        .iter()
        .copied()
        .filter(|&edge| mesh.contains_edge(edge))
        .collect();
    let mut collapsing: IdSet<EdgeId> = IdSet::new();
    let all: Vec<EdgeId> = mesh.edges().collect();
    for &edge in &all {
        if marked.contains(edge) && mesh.edge_length_squared(edge) < distance_squared {
            collapsing.insert(edge);
        }
        let loops: Vec<LoopId> = mesh.edge_loops(edge).collect();
        for lp in loops {
            mesh.set_loop_tag(lp, false);
        }
    }
    if !collapsing.is_empty() {
        let list: Vec<EdgeId> = collapsing.iter().collect();
        collapse_geometry(mesh, &list, true);
    }
    marked.retain(|edge| mesh.contains_edge(edge));
    collapsing.clear();

    let mut found = false;
    let all: Vec<EdgeId> = mesh.edges().collect();
    for edge in all {
        if !marked.contains(edge) || !mesh.contains_edge(edge) {
            continue;
        }
        let Some(start) = mesh.edge_first_loop(edge) else {
            continue;
        };
        let mut first = start;
        let mut lp = first;
        loop {
            let fresh = !mesh.is_loop_tagged(lp);
            if fresh {
                mesh.set_loop_tag(lp, true);
            }
            let prev = mesh.loop_prev(lp);
            if fresh
                && marked.contains(mesh.loop_edge(prev))
                && !collapsing.contains(mesh.loop_edge(lp))
                && !collapsing.contains(mesh.loop_edge(prev))
                && clip_ear(mesh, lp, distance, &mut collapsing, &mut found)
            {
                first = lp;
            }
            lp = mesh.loop_radial_next(lp);
            if lp == first {
                break;
            }
        }
    }
    if found {
        let list: Vec<EdgeId> = collapsing.iter().collect();
        collapse_geometry(mesh, &list, true);
    }
}

fn clip_ear(
    mesh: &mut EditMesh,
    lp: LoopId,
    distance: f32,
    collapsing: &mut IdSet<EdgeId>,
    found: &mut bool,
) -> bool {
    let center = mesh.position(mesh.loop_vert(lp));
    let prev = mesh.loop_prev(lp);
    let next = mesh.loop_next(lp);
    let (dir_prev, len_prev) = normalize_with_length(mesh.position(mesh.loop_vert(prev)) - center);
    let (dir_next, len_next) = normalize_with_length(mesh.position(mesh.loop_vert(next)) - center);
    if dir_prev.distance(dir_next) * len_prev.min(len_next) > distance {
        return false;
    }
    let face = mesh.loop_face(lp);
    if (len_prev - len_next).abs() <= distance {
        if mesh.face_len(face) == 3 {
            collapsing.insert(mesh.loop_edge(next));
            *found = true;
            return false;
        }
        if let Ok(split) = mesh.split_face(face, prev, next, None, true) {
            collapsing.insert(mesh.loop_edge(split.lp));
            *found = true;
            return true;
        }
        return false;
    }
    let (edge, factor) = match len_prev < len_next {
        true => (mesh.loop_edge(lp), len_prev / len_next),
        false => (mesh.loop_edge(prev), len_next / len_prev),
    };
    if mesh.split_edge(edge, mesh.loop_vert(lp), factor).is_err() {
        return true;
    }
    let prev = mesh.loop_prev(lp);
    let next = mesh.loop_next(lp);
    if let Ok(split) = mesh.split_face(face, prev, next, None, true) {
        collapsing.insert(mesh.loop_edge(split.lp));
        *found = true;
    }
    true
}
