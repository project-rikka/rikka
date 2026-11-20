use std::collections::VecDeque;
use std::f32::consts::PI;

use rikka_mesh_edit::{
    DeleteContext, DoubleFace, EdgeId, EditMesh, FaceId, Geometry, IdSet, SelectMode, VertId,
};

use crate::common::{
    Counts, Removed, clear_uv_selection, selected_edges, selected_faces, selected_verts,
};
use crate::error::{Result, check_range};
use crate::euler::{collapse_vert_merging, split_corners};
use crate::math::{angle_at, angle_on_axis, interpf};

const ANGLE_EPSILON: f32 = 0.0001 * (180.0 / PI);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DissolveVertsOptions {
    pub use_face_split: bool,
    pub use_boundary_tear: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DissolveEdgesOptions {
    pub use_verts: bool,
    pub angle_threshold: f32,
    pub use_face_split: bool,
    pub use_preserve_quads: bool,
}

impl Default for DissolveEdgesOptions {
    fn default() -> DissolveEdgesOptions {
        DissolveEdgesOptions {
            use_verts: true,
            angle_threshold: PI,
            use_face_split: false,
            use_preserve_quads: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DissolveFacesOptions {
    pub use_verts: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DissolveModeOptions {
    pub use_verts: Option<bool>,
    pub angle_threshold: f32,
    pub use_preserve_quads: bool,
    pub use_face_split: bool,
    pub use_boundary_tear: bool,
}

impl Default for DissolveModeOptions {
    fn default() -> DissolveModeOptions {
        DissolveModeOptions {
            use_verts: None,
            angle_threshold: PI,
            use_preserve_quads: true,
            use_face_split: false,
            use_boundary_tear: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DeleteEdgeLoopOptions {
    pub use_face_split: bool,
}

impl Default for DeleteEdgeLoopOptions {
    fn default() -> DeleteEdgeLoopOptions {
        DeleteEdgeLoopOptions {
            use_face_split: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DissolveOutput {
    pub removed: Removed,
    pub region: IdSet<FaceId>,
}

pub fn dissolve_verts(mesh: &mut EditMesh, options: &DissolveVertsOptions) -> Result<Removed> {
    let counts = Counts::new(mesh);
    let verts = selected_verts(mesh);
    if verts.is_empty() {
        return Ok(counts.removed(mesh));
    }
    dissolve_verts_geometry(mesh, &verts, options);
    mesh.update_normals();
    Ok(counts.removed(mesh))
}

pub fn dissolve_verts_geometry(
    mesh: &mut EditMesh,
    verts: &[VertId],
    options: &DissolveVertsOptions,
) {
    let verts: Vec<VertId> = verts
        .iter()
        .copied()
        .filter(|&vert| mesh.contains_vert(vert))
        .collect();
    let marked: IdSet<VertId> = verts.iter().copied().collect();
    let mut loose_verts: IdSet<VertId> = marked.clone();
    let mut loose_edges: IdSet<EdgeId> = IdSet::new();

    if options.use_face_split {
        split_corners(mesh, &marked, false);
    }
    if options.use_boundary_tear {
        let torn: IdSet<VertId> = verts
            .iter()
            .copied()
            .filter(|&vert| {
                !mesh.is_vert_edge_pair(vert)
                    && mesh
                        .vert_edges(vert)
                        .any(|edge| mesh.is_edge_boundary(edge))
            })
            .collect();
        split_corners(mesh, &torn, true);
    }

    for &vert in &verts {
        let mut first_edge: Option<EdgeId> = None;
        let corners = mesh.vert_loops(vert).collect::<Vec<_>>();
        for corner in corners {
            for lp in mesh.loop_cycle(corner) {
                loose_verts.insert(mesh.loop_vert(lp));
                loose_edges.insert(mesh.loop_edge(lp));
            }
            first_edge = Some(mesh.loop_edge(corner));
        }
        let Some(first) = first_edge else {
            continue;
        };
        let mut edge = first;
        loop {
            let next = mesh.disk_next(edge, vert);
            if mesh.is_edge_wire(edge) {
                mesh.kill_edge(edge).ok();
            }
            edge = next;
            if edge == first {
                break;
            }
        }
    }

    let pairs: IdSet<VertId> = verts
        .iter()
        .copied()
        .filter(|&vert| mesh.is_vert_edge_pair(vert))
        .collect();

    for &vert in &verts {
        if pairs.contains(vert) {
            continue;
        }
        let edges: Vec<EdgeId> = mesh.vert_edges(vert).collect();
        for edge in edges {
            if !mesh.contains_edge(edge) {
                continue;
            }
            if let Some((a, b)) = mesh.edge_loop_pair(edge) {
                mesh.join_face_pair_keep_shared(a, b, DoubleFace::Reuse)
                    .ok();
            }
        }
    }

    kill_loose_edges(mesh, &loose_edges);

    for &vert in &verts {
        if mesh.contains_vert(vert) && mesh.is_vert_edge_pair(vert) {
            collapse_vert_merging(mesh, vert);
        }
    }

    kill_loose_verts(mesh, &loose_verts);
}

fn kill_loose_edges(mesh: &mut EditMesh, candidates: &IdSet<EdgeId>) {
    let edges: Vec<EdgeId> = candidates.iter().collect();
    for edge in edges {
        if mesh.contains_edge(edge) && mesh.edge_first_loop(edge).is_none() {
            mesh.kill_edge(edge).ok();
        }
    }
}

fn kill_loose_verts(mesh: &mut EditMesh, candidates: &IdSet<VertId>) {
    let verts: Vec<VertId> = candidates.iter().collect();
    for vert in verts {
        if mesh.contains_vert(vert) && mesh.vert_first_edge(vert).is_none() {
            mesh.kill_vert(vert).ok();
        }
    }
}

pub fn dissolve_edges(mesh: &mut EditMesh, options: &DissolveEdgesOptions) -> Result<Removed> {
    check_range(options.angle_threshold, 0.0, PI, "angle_threshold")?;
    let counts = Counts::new(mesh);
    let edges = selected_edges(mesh);
    if edges.is_empty() {
        return Ok(counts.removed(mesh));
    }
    dissolve_edges_geometry(mesh, &edges, options);
    mesh.update_normals();
    Ok(counts.removed(mesh))
}

fn edge_angle_blended(mesh: &EditMesh, vert: VertId) -> f32 {
    let Some((first, second)) = mesh.vert_edge_pair(vert) else {
        return 0.0;
    };
    let center = mesh.position(vert);
    let a = mesh.position(mesh.edge_other_vert(first, vert).unwrap_or(vert));
    let b = mesh.position(mesh.edge_other_vert(second, vert).unwrap_or(vert));
    let mut angle = PI - angle_at(a, center, b);
    if let Some((face_a, face_b)) = mesh.edge_face_pair(first) {
        let normal_a = mesh.calc_face_normal(face_a);
        let normal_b = mesh.calc_face_normal(face_b);
        let raw_factor = (-normal_a.dot(normal_b)).clamp(0.0, 1.0);
        let normal_angle = PI - angle_on_axis(a, center, b, mesh.vert_normal(vert));
        angle = interpf(angle, normal_angle, raw_factor);
    }
    angle
}

fn find_end_of_chain(
    mesh: &EditMesh,
    mut edge: EdgeId,
    mut vert: VertId,
    mut flagged: Option<&mut IdSet<EdgeId>>,
) -> Option<VertId> {
    let start = vert;
    while mesh.is_vert_edge_pair(vert) {
        edge = mesh.disk_next(edge, vert);
        vert = mesh.edge_other_vert(edge, vert)?;
        if let Some(flagged) = flagged.as_deref_mut()
            && !flagged.insert(edge)
        {
            return None;
        }
        if vert == start {
            return None;
        }
    }
    Some(vert)
}

fn tagged_edge_count(mesh: &EditMesh, vert: VertId, tagged: &IdSet<EdgeId>, max: usize) -> usize {
    let mut count = 0;
    for edge in mesh.vert_edges(vert) {
        if tagged.contains(edge) {
            count += 1;
        }
        if count == max {
            break;
        }
    }
    count
}

fn touches_unselected_face(
    mesh: &EditMesh,
    vert: VertId,
    marked: &IdSet<VertId>,
    tagged: &IdSet<EdgeId>,
) -> bool {
    if marked.contains(vert) {
        return false;
    }
    mesh.vert_loops(vert).any(|lp| {
        let other = mesh.loop_other_edge_loop(lp, vert);
        !tagged.contains(mesh.loop_edge(lp)) && !tagged.contains(mesh.loop_edge(other))
    })
}

pub fn dissolve_edges_geometry(
    mesh: &mut EditMesh,
    edges: &[EdgeId],
    options: &DissolveEdgesOptions,
) {
    let edges: Vec<EdgeId> = edges
        .iter()
        .copied()
        .filter(|&edge| mesh.contains_edge(edge))
        .collect();
    let threshold = options.angle_threshold;
    let use_verts = options.use_verts && threshold > ANGLE_EPSILON;
    let dissolve_all = threshold > PI - ANGLE_EPSILON;

    let mut tagged: IdSet<EdgeId> = IdSet::new();
    let mut chain: IdSet<EdgeId> = IdSet::new();
    let mut loose_edges: IdSet<EdgeId> = IdSet::new();
    let mut loose_verts: IdSet<VertId> = IdSet::new();
    let mut marked: IdSet<VertId> = IdSet::new();

    if options.use_face_split || use_verts {
        tagged.extend(edges.iter().copied());
    }

    for &edge in &edges {
        let [v1, v2] = mesh.edge_verts(edge);
        if mesh.is_vert_edge_pair(v1) || mesh.is_vert_edge_pair(v2) {
            chain.insert(edge);
        }
        if let Some((face_a, face_b)) = mesh.edge_face_pair(edge) {
            for face in [face_a, face_b] {
                for lp in mesh.face_loops(face) {
                    loose_verts.insert(mesh.loop_vert(lp));
                    loose_edges.insert(mesh.loop_edge(lp));
                }
            }
            if use_verts && chain.contains(edge) {
                find_end_of_chain(mesh, edge, v1, Some(&mut tagged));
                find_end_of_chain(mesh, edge, v2, Some(&mut tagged));
            }
        }
    }

    if use_verts {
        for &edge in &edges {
            let Some((face_a, face_b)) = mesh.edge_face_pair(edge) else {
                continue;
            };
            for start in mesh.edge_verts(edge) {
                if options.use_preserve_quads
                    && mesh.face_len(face_a) == 3
                    && mesh.face_len(face_b) == 3
                    && tagged_edge_count(mesh, start, &tagged, 2) == 1
                {
                    continue;
                }
                let end = match mesh.is_vert_edge_pair(start) {
                    true => find_end_of_chain(mesh, edge, start, Some(&mut chain)),
                    false => Some(start),
                };
                let Some(end) = end else {
                    continue;
                };
                if touches_unselected_face(mesh, end, &marked, &tagged)
                    && tagged_edge_count(mesh, end, &tagged, 2) != 1
                {
                    continue;
                }
                marked.insert(end);
            }
        }
    }

    if options.use_face_split {
        let corners: IdSet<VertId> = mesh
            .verts()
            .filter(|&vert| {
                mesh.vert_edges(vert)
                    .filter(|&edge| !tagged.contains(edge))
                    .count()
                    <= 2
            })
            .collect();
        split_corners(mesh, &corners, false);
    }

    for &edge in &edges {
        if !mesh.contains_edge(edge) {
            continue;
        }
        if let Some((a, b)) = mesh.edge_loop_pair(edge) {
            mesh.join_face_pair_keep_shared(a, b, DoubleFace::Reuse)
                .ok();
        }
    }

    kill_loose_edges(mesh, &loose_edges);
    kill_loose_verts(mesh, &loose_verts);

    if use_verts {
        let candidates: Vec<VertId> = marked.iter().collect();
        for vert in candidates {
            if !mesh.contains_vert(vert) {
                marked.remove(vert);
                continue;
            }
            let Some((first, second)) = mesh.vert_edge_pair(vert) else {
                marked.remove(vert);
                continue;
            };
            if dissolve_all || chain.contains(first) || chain.contains(second) {
                continue;
            }
            if edge_angle_blended(mesh, vert) > threshold - ANGLE_EPSILON {
                marked.remove(vert);
            }
        }
        let collapsing: Vec<VertId> = marked.iter().collect();
        for vert in collapsing {
            if mesh.contains_vert(vert) && mesh.is_vert_edge_pair(vert) {
                collapse_vert_merging(mesh, vert);
            }
        }
    }
}

pub fn dissolve_faces(
    mesh: &mut EditMesh,
    options: &DissolveFacesOptions,
) -> Result<DissolveOutput> {
    let counts = Counts::new(mesh);
    let faces = selected_faces(mesh);
    if faces.is_empty() {
        return Ok(DissolveOutput {
            removed: counts.removed(mesh),
            region: IdSet::new(),
        });
    }
    let region = dissolve_faces_geometry(mesh, &faces, options);
    for face in region.iter() {
        mesh.select_face(face, true);
    }
    mesh.update_normals();
    Ok(DissolveOutput {
        removed: counts.removed(mesh),
        region,
    })
}

fn manifold_island(
    mesh: &EditMesh,
    start: FaceId,
    mask: &IdSet<FaceId>,
    visited: &mut IdSet<FaceId>,
) -> Vec<FaceId> {
    let mut island = Vec::new();
    let mut queue: VecDeque<FaceId> = VecDeque::new();
    visited.insert(start);
    queue.push_back(start);
    while let Some(face) = queue.pop_front() {
        island.push(face);
        for lp in mesh.face_loops(face) {
            let mut radial = mesh.loop_radial_next(lp);
            if radial != lp {
                let mut count = 1;
                while radial != lp && count < 3 {
                    if mask.contains(mesh.loop_face(radial)) {
                        count += 1;
                    }
                    radial = mesh.loop_radial_next(radial);
                }
                if count != 2 {
                    continue;
                }
            }
            let mut radial = mesh.loop_radial_next(lp);
            while radial != lp {
                let other = mesh.loop_face(radial);
                if mask.contains(other) && other != face && !visited.contains(other) {
                    visited.insert(other);
                    queue.push_back(other);
                    break;
                }
                radial = mesh.loop_radial_next(radial);
            }
        }
    }
    island
}

pub fn dissolve_faces_geometry(
    mesh: &mut EditMesh,
    faces: &[FaceId],
    options: &DissolveFacesOptions,
) -> IdSet<FaceId> {
    let faces: Vec<FaceId> = faces
        .iter()
        .copied()
        .filter(|&face| mesh.contains_face(face))
        .collect();
    let keep_verts: IdSet<VertId> = match options.use_verts {
        true => mesh
            .verts()
            .filter(|&vert| !mesh.is_vert_edge_pair(vert))
            .collect(),
        false => IdSet::new(),
    };
    let input: IdSet<FaceId> = faces.iter().copied().collect();
    let mut pending = input.clone();
    let mut original: IdSet<FaceId> = IdSet::new();
    let mut regions: Vec<Vec<FaceId>> = Vec::new();
    let mut visited: IdSet<FaceId> = IdSet::new();
    for &face in &faces {
        if !pending.contains(face) || visited.contains(face) {
            continue;
        }
        let island = manifold_island(mesh, face, &input, &mut visited);
        if island.len() >= 2 {
            for &member in &island {
                pending.remove(member);
                original.insert(member);
            }
            regions.push(island);
        }
    }

    let mut target = mesh.face_count();
    let mut region_out: IdSet<FaceId> = IdSet::new();
    for region in regions {
        match mesh.join_faces(&region, DoubleFace::Keep).ok() {
            Some(joined) => {
                target -= region.len() - 1;
                let mut face = joined.face;
                if let Some(double) = joined.double {
                    if mesh.kill_face(face).is_ok() {
                        target -= 1;
                    }
                    face = double;
                }
                original.remove(face);
                region_out.insert(face);
            }
            None => {
                for &member in &region {
                    original.remove(member);
                }
            }
        }
    }

    if target != mesh.face_count() {
        let mut doomed = Geometry::new();
        doomed.faces = original
            .iter()
            .filter(|&face| mesh.contains_face(face))
            .collect();
        mesh.delete(DeleteContext::Faces, &doomed);
    }

    if options.use_verts {
        let verts: Vec<VertId> = mesh.verts().collect();
        for vert in verts {
            if keep_verts.contains(vert) && mesh.contains_vert(vert) && mesh.is_vert_edge_pair(vert)
            {
                collapse_vert_merging(mesh, vert);
            }
        }
    }

    region_out.retain(|face| mesh.contains_face(face));
    region_out
}

pub fn dissolve_mode(mesh: &mut EditMesh, options: &DissolveModeOptions) -> Result<DissolveOutput> {
    let mode = mesh.select_mode();
    let use_verts = options
        .use_verts
        .unwrap_or(!mode.contains(SelectMode::FACE));
    if mode.contains(SelectMode::VERT) {
        let removed = dissolve_verts(
            mesh,
            &DissolveVertsOptions {
                use_face_split: options.use_face_split,
                use_boundary_tear: options.use_boundary_tear,
            },
        )?;
        return Ok(DissolveOutput {
            removed,
            region: IdSet::new(),
        });
    }
    if mode.contains(SelectMode::EDGE) {
        let removed = dissolve_edges(
            mesh,
            &DissolveEdgesOptions {
                use_verts,
                angle_threshold: options.angle_threshold,
                use_face_split: options.use_face_split,
                use_preserve_quads: options.use_preserve_quads,
            },
        )?;
        return Ok(DissolveOutput {
            removed,
            region: IdSet::new(),
        });
    }
    dissolve_faces(mesh, &DissolveFacesOptions { use_verts })
}

pub fn delete_edge_loop(mesh: &mut EditMesh, options: &DeleteEdgeLoopOptions) -> Result<Removed> {
    let counts = Counts::new(mesh);
    let edges = selected_edges(mesh);
    if edges.is_empty() {
        return Ok(counts.removed(mesh));
    }
    let faces: Vec<FaceId> = mesh.faces().collect();
    for face in faces {
        mesh.set_face_tag(face, false);
    }
    for &edge in &edges {
        let around: Vec<FaceId> = mesh.edge_faces(edge).collect();
        for face in around {
            mesh.set_face_tag(face, true);
        }
    }
    dissolve_edges_geometry(
        mesh,
        &edges,
        &DissolveEdgesOptions {
            use_verts: true,
            angle_threshold: PI,
            use_face_split: options.use_face_split,
            use_preserve_quads: false,
        },
    );
    let tagged: Vec<FaceId> = mesh
        .faces()
        .filter(|&face| mesh.is_face_tagged(face) && !mesh.is_face_hidden(face))
        .collect();
    for face in tagged {
        mesh.select_face(face, true);
    }
    mesh.select_mode_flush_with(SelectMode::VERT, false);
    clear_uv_selection(mesh);
    mesh.update_normals();
    Ok(counts.removed(mesh))
}
