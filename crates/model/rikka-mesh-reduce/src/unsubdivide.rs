use rikka_mesh_edit::{EdgeId, EditMesh, IdSet, LocalSet, LoopId, SelectMode, VertId};
use smallvec::SmallVec;

use crate::common::{Counts, Removed};
use crate::error::{Error, Result};
use crate::euler::collapse_vert;

const DISSOLVE_MAX: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnsubdivideOptions {
    pub iterations: u32,
}

impl Default for UnsubdivideOptions {
    fn default() -> UnsubdivideOptions {
        UnsubdivideOptions { iterations: 2 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Method {
    InteriorFan,
    InteriorChain,
    BoundaryFan,
    WireChain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Collapse,
    Init,
    Ignore,
}

type Loops = SmallVec<[LoopId; DISSOLVE_MAX]>;

pub fn unsubdivide(mesh: &mut EditMesh, options: &UnsubdivideOptions) -> Result<Removed> {
    if options.iterations == 0 || options.iterations > 1000 {
        return Err(Error::OutOfRange("iterations"));
    }
    let counts = Counts::new(mesh);
    let any = mesh.verts().any(|vert| mesh.is_vert_selected(vert))
        || mesh.edges().any(|edge| mesh.is_edge_selected(edge))
        || mesh.faces().any(|face| mesh.is_face_selected(face));
    if !any {
        return Ok(counts.removed(mesh));
    }
    let verts: Vec<VertId> = mesh
        .verts()
        .filter(|&vert| mesh.is_vert_selected(vert) && !mesh.is_vert_hidden(vert))
        .collect();
    unsubdivide_geometry(mesh, &verts, options.iterations);
    let mode = mesh.select_mode();
    if !mode.contains(SelectMode::VERT) {
        mesh.select_mode_flush_with(SelectMode::VERT, false);
    }
    mesh.select_mode_flush();
    mesh.update_normals();
    Ok(counts.removed(mesh))
}

pub fn decimate_unsubdivide(mesh: &mut EditMesh, iterations: u32) {
    let verts: Vec<VertId> = mesh.verts().collect();
    unsubdivide_geometry(mesh, &verts, iterations);
    mesh.update_normals();
}

pub fn unsubdivide_geometry(mesh: &mut EditMesh, verts: &[VertId], iterations: u32) {
    let iterations = iterations.max(1);
    let tagged: IdSet<VertId> = verts
        .iter()
        .copied()
        .filter(|&vert| mesh.contains_vert(vert))
        .collect();
    let mut marks: Vec<Mark> = Vec::new();
    let mut ignore: Vec<VertId> = Vec::new();
    let mut collapse: Vec<VertId> = Vec::new();
    for _ in 0..iterations {
        marks.clear();
        marks.resize(mesh.vert_capacity(), Mark::Ignore);
        let all: Vec<VertId> = mesh.verts().collect();
        for &vert in &all {
            if tagged.contains(vert) && dissolve_test(mesh, vert, false).is_some() {
                marks[vert.index()] = Mark::Init;
            }
        }
        let mut marked = false;
        for &vert in &all {
            if marks[vert.index()] != Mark::Init {
                continue;
            }
            marks[vert.index()] = Mark::Ignore;
            ignore.clear();
            ignore.push(vert);
            loop {
                tag_neighbours(mesh, &ignore, Mark::Collapse, &mut marks, &mut collapse);
                if collapse.is_empty() {
                    break;
                }
                marked = true;
                tag_neighbours(mesh, &collapse, Mark::Ignore, &mut marks, &mut ignore);
                if ignore.is_empty() {
                    break;
                }
            }
        }
        if !marked {
            break;
        }
        for &vert in &all {
            if mesh.contains_vert(vert) && marks[vert.index()] == Mark::Collapse {
                dissolve_fan_or_chain(mesh, vert);
            }
        }
    }
}

fn tag_neighbours(
    mesh: &EditMesh,
    start: &[VertId],
    mark: Mark,
    marks: &mut [Mark],
    tagged: &mut Vec<VertId>,
) {
    tagged.clear();
    for &vert in start {
        for edge in mesh.vert_edges(vert) {
            let Some(other) = mesh.edge_other_vert(edge, vert) else {
                continue;
            };
            if marks[other.index()] == Mark::Init {
                marks[other.index()] = mark;
                tagged.push(other);
            }
        }
    }
}

fn vert_loops(mesh: &EditMesh, vert: VertId, edges: &[EdgeId]) -> Loops {
    let mut loops = Loops::new();
    for &edge in edges {
        for lp in mesh.edge_loops(edge) {
            if mesh.loop_vert(lp) == vert {
                if loops.len() == DISSOLVE_MAX {
                    return loops;
                }
                loops.push(lp);
            }
        }
    }
    loops
}

fn makes_double_whole_face(mesh: &EditMesh, loops: &[LoopId]) -> bool {
    let mut opposite: SmallVec<[EdgeId; DISSOLVE_MAX]> = SmallVec::new();
    for &lp in loops {
        let edge = match mesh.face_len(mesh.loop_face(lp)) > 3 {
            true => {
                let prev = mesh.loop_vert(mesh.loop_prev(lp));
                let next = mesh.loop_vert(mesh.loop_next(lp));
                let Some(edge) = mesh.edge_between(prev, next) else {
                    return false;
                };
                if opposite.len() == DISSOLVE_MAX {
                    return false;
                }
                edge
            }
            false => mesh.loop_edge(mesh.loop_next(lp)),
        };
        opposite.push(edge);
    }
    opposite.len() >= 3 && face_exists_multi_edge(mesh, &opposite)
}

fn face_exists_multi_edge(mesh: &EditMesh, edges: &[EdgeId]) -> bool {
    let mut verts: SmallVec<[VertId; DISSOLVE_MAX]> = SmallVec::new();
    let mut pending: LocalSet<VertId> = LocalSet::new();
    for &edge in edges {
        pending.extend(mesh.edge_verts(edge));
    }
    for &edge in edges {
        for vert in mesh.edge_verts(edge) {
            if pending.remove(vert) {
                verts.push(vert);
            }
        }
    }
    if verts.len() != edges.len() {
        return false;
    }
    let vert_set: LocalSet<VertId> = verts.iter().copied().collect();
    let edge_set: LocalSet<EdgeId> = edges.iter().copied().collect();
    let mut faces: LocalSet<rikka_mesh_edit::FaceId> = LocalSet::new();
    for &edge in edges {
        for face in mesh.edge_faces(edge) {
            if faces.contains(face) {
                continue;
            }
            if mesh.face_verts(face).all(|vert| vert_set.contains(vert)) {
                faces.insert(face);
            }
        }
    }
    if faces.is_empty() {
        return false;
    }
    for &vert in &verts {
        for edge in mesh.vert_edges(vert) {
            let [a, b] = mesh.edge_verts(edge);
            if !edge_set.contains(edge) && vert_set.contains(a) && vert_set.contains(b) {
                let count = mesh
                    .edge_faces(edge)
                    .filter(|&face| faces.contains(face))
                    .count();
                if count != 2 {
                    return false;
                }
            }
        }
    }
    true
}

fn split_makes_double(mesh: &EditMesh, lp: LoopId) -> bool {
    let prev = mesh.loop_prev(lp);
    let next = mesh.loop_next(lp);
    let triangle = [
        mesh.loop_vert(prev),
        mesh.loop_vert(lp),
        mesh.loop_vert(next),
    ];
    if mesh.face_exists(&triangle).is_some() {
        return true;
    }
    let mut rest: Vec<VertId> = Vec::new();
    let mut walk = next;
    loop {
        rest.push(mesh.loop_vert(walk));
        if walk == prev {
            break;
        }
        walk = mesh.loop_next(walk);
    }
    mesh.face_exists(&rest).is_some()
}

fn makes_double_with_existing(mesh: &EditMesh, loops: &[LoopId]) -> bool {
    loops
        .iter()
        .any(|&lp| mesh.face_len(mesh.loop_face(lp)) > 3 && split_makes_double(mesh, lp))
}

fn canonical_collapsed(mesh: &EditMesh, corner: LoopId) -> Vec<VertId> {
    let mut verts: Vec<VertId> = Vec::new();
    let mut lp = mesh.loop_next(corner);
    while lp != corner {
        verts.push(mesh.loop_vert(lp));
        lp = mesh.loop_next(lp);
    }
    let count = verts.len();
    let mut min = 0;
    for index in 1..count {
        if verts[index] < verts[min] {
            min = index;
        }
    }
    let next = (min + 1) % count;
    let prev = (min + count - 1) % count;
    let step = match verts[next] < verts[prev] {
        true => 1,
        false => count - 1,
    };
    let mut ordered = Vec::with_capacity(count);
    let mut index = min;
    for _ in 0..count {
        ordered.push(verts[index]);
        index = (index + step) % count;
    }
    ordered
}

fn makes_double_with_self(mesh: &EditMesh, loops: &[LoopId]) -> bool {
    if loops.len() < 2 {
        return false;
    }
    let mut large: Loops = loops
        .iter()
        .copied()
        .filter(|&lp| mesh.face_len(mesh.loop_face(lp)) > 3)
        .collect();
    if large.len() < 2 {
        return false;
    }
    large.sort_by_key(|&lp| mesh.face_len(mesh.loop_face(lp)));
    let mut begin = 0;
    while begin < large.len() {
        let len = mesh.face_len(mesh.loop_face(large[begin]));
        let mut end = begin + 1;
        while end < large.len() && mesh.face_len(mesh.loop_face(large[end])) == len {
            end += 1;
        }
        if end - begin >= 2 {
            let collapsed: Vec<Vec<VertId>> = large[begin..end]
                .iter()
                .map(|&lp| canonical_collapsed(mesh, lp))
                .collect();
            for first in 0..collapsed.len() {
                for second in first + 1..collapsed.len() {
                    if collapsed[first] == collapsed[second] {
                        return true;
                    }
                }
            }
        }
        begin = end;
    }
    false
}

fn dissolve_test(mesh: &EditMesh, vert: VertId, check_duplicates: bool) -> Option<(Method, Loops)> {
    let mut edges: SmallVec<[EdgeId; DISSOLVE_MAX]> = SmallVec::new();
    let mut boundary: SmallVec<[EdgeId; 2]> = SmallVec::new();
    let (mut boundary_count, mut manifold_count, mut wire_count) = (0, 0, 0);
    for edge in mesh.vert_edges(vert) {
        if mesh.is_edge_boundary(edge) {
            if boundary.len() < 2 {
                boundary.push(edge);
            }
            boundary_count += 1;
        } else if mesh.is_edge_manifold(edge) {
            manifold_count += 1;
        } else if mesh.is_edge_wire(edge) {
            wire_count += 1;
        } else {
            return None;
        }
        if edges.len() == DISSOLVE_MAX {
            return None;
        }
        edges.push(edge);
    }
    let total = edges.len();
    if (total == 4 && boundary_count == 0 && manifold_count == 4)
        || (total == 3 && boundary_count == 0 && manifold_count == 3)
    {
        let mut loops = Loops::new();
        if check_duplicates {
            loops = vert_loops(mesh, vert, &edges);
            if makes_double_whole_face(mesh, &loops)
                || makes_double_with_existing(mesh, &loops)
                || makes_double_with_self(mesh, &loops)
            {
                return None;
            }
        }
        return Some((Method::InteriorFan, loops));
    }
    if total == 3 && boundary_count == 2 && manifold_count == 1 {
        let mut loops = Loops::new();
        if check_duplicates {
            let a = mesh.edge_other_vert(boundary[0], vert)?;
            let b = mesh.edge_other_vert(boundary[1], vert)?;
            if let Some(span) = mesh.edge_between(a, b)
                && mesh.edge_first_loop(span).is_some()
            {
                return None;
            }
            loops = vert_loops(mesh, vert, &edges);
            if makes_double_with_existing(mesh, &loops) || makes_double_with_self(mesh, &loops) {
                return None;
            }
        }
        return Some((Method::BoundaryFan, loops));
    }
    if total == 2 && wire_count == 2 {
        return Some((Method::WireChain, Loops::new()));
    }
    if total == 2 && manifold_count == 2 {
        return Some((Method::InteriorChain, Loops::new()));
    }
    None
}

fn dissolve_fan_or_chain(mesh: &mut EditMesh, vert: VertId) -> bool {
    let Some((method, loops)) = dissolve_test(mesh, vert, true) else {
        return false;
    };
    match method {
        Method::WireChain | Method::InteriorChain => collapse_vert(mesh, vert).is_some(),
        Method::BoundaryFan | Method::InteriorFan => {
            for lp in loops {
                let face = mesh.loop_face(lp);
                if mesh.face_len(face) <= 3 {
                    continue;
                }
                let prev = mesh.loop_prev(lp);
                let next = mesh.loop_next(lp);
                if let Ok(split) = mesh.split_face(face, prev, next, None, true) {
                    let created = mesh.loop_edge(split.lp);
                    let flags = mesh
                        .edge_flags(mesh.loop_edge(lp))
                        .merge_hidden(mesh.edge_flags(mesh.loop_edge(mesh.loop_prev(lp))));
                    mesh.set_edge_flags(created, flags);
                }
            }
            mesh.dissolve_vert(vert).is_ok()
        }
    }
}
