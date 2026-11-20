use bitflags::bitflags;
use glam::Vec2;
use rikka_mesh_edit::{
    DoubleFace, EdgeId, EditMesh, FaceId, IdMap, IdSet, MathLayer, MathLayerKind, SelectMode,
    VertId,
};

use crate::common::{Counts, selected_edges, selected_verts};
use crate::dissolve::DissolveOutput;
use crate::error::{Result, check_range};
use crate::euler::collapse_vert;
use crate::math::{
    AxisProjection, RIGHT_ANGLE, angle_at, angle_normalized, cos_at, cross_tri_2d,
    is_point_in_tri_cw,
};
use rikka_mesh_edit::{Heap, HeapNode};

const COST_INVALID: f32 = f32::MAX;

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Delimit: u8 {
        const NORMAL = 1;
        const MATERIAL = 1 << 1;
        const SEAM = 1 << 2;
        const SHARP = 1 << 3;
        const UV = 1 << 4;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LimitedDissolveOptions {
    pub angle_limit: f32,
    pub use_dissolve_boundaries: bool,
    pub delimit: Delimit,
}

impl Default for LimitedDissolveOptions {
    fn default() -> LimitedDissolveOptions {
        LimitedDissolveOptions {
            angle_limit: 5.0f32.to_radians(),
            use_dissolve_boundaries: false,
            delimit: Delimit::NORMAL,
        }
    }
}

pub fn dissolve_limited(
    mesh: &mut EditMesh,
    options: &LimitedDissolveOptions,
) -> Result<DissolveOutput> {
    check_range(
        options.angle_limit,
        0.0,
        std::f32::consts::PI,
        "angle_limit",
    )?;
    let counts = Counts::new(mesh);
    let any = mesh.verts().any(|vert| mesh.is_vert_selected(vert))
        || mesh.edges().any(|edge| mesh.is_edge_selected(edge))
        || mesh.faces().any(|face| mesh.is_face_selected(face));
    if !any {
        return Ok(DissolveOutput {
            removed: counts.removed(mesh),
            region: IdSet::new(),
        });
    }
    let (verts, edges) = match mesh.select_mode() == SelectMode::FACE {
        true => face_mode_input(mesh),
        false => (selected_verts(mesh), selected_edges(mesh)),
    };
    let region = dissolve_limited_geometry(
        mesh,
        &verts,
        &edges,
        options.angle_limit.min(RIGHT_ANGLE),
        options.use_dissolve_boundaries,
        options.delimit,
    );
    for face in region.iter() {
        mesh.select_face(face, true);
    }
    mesh.update_normals();
    Ok(DissolveOutput {
        removed: counts.removed(mesh),
        region,
    })
}

fn face_mode_input(mesh: &EditMesh) -> (Vec<VertId>, Vec<EdgeId>) {
    let mut verts: IdSet<VertId> = mesh
        .verts()
        .filter(|&vert| mesh.is_vert_selected(vert))
        .collect();
    let mut edges: IdSet<EdgeId> = mesh
        .edges()
        .filter(|&edge| mesh.is_edge_selected(edge))
        .collect();
    for face in mesh.faces() {
        if !mesh.is_face_selected(face) {
            for lp in mesh.face_loops(face) {
                verts.remove(mesh.loop_vert(lp));
                edges.remove(mesh.loop_edge(lp));
            }
        }
    }
    (
        verts
            .iter()
            .filter(|&vert| !mesh.is_vert_hidden(vert))
            .collect(),
        edges
            .iter()
            .filter(|&edge| !mesh.is_edge_hidden(edge))
            .collect(),
    )
}

pub fn decimate_planar(
    mesh: &mut EditMesh,
    angle_limit: f32,
    use_dissolve_boundaries: bool,
    delimit: Delimit,
) {
    let verts: Vec<VertId> = mesh.verts().collect();
    let edges: Vec<EdgeId> = mesh.edges().collect();
    dissolve_limited_geometry(
        mesh,
        &verts,
        &edges,
        angle_limit,
        use_dissolve_boundaries,
        delimit,
    );
    mesh.update_normals();
}

struct Delimiter {
    delimit: Delimit,
    uv_layers: Vec<MathLayer>,
}

impl Delimiter {
    fn new(mesh: &EditMesh, mut delimit: Delimit) -> Delimiter {
        let uv_layers: Vec<MathLayer> = mesh
            .loop_math_layers()
            .into_iter()
            .filter(|layer| layer.kind == MathLayerKind::Vec2)
            .collect();
        if uv_layers.is_empty() {
            delimit.remove(Delimit::UV);
        }
        Delimiter { delimit, uv_layers }
    }

    fn is_edge_delimiter(&self, mesh: &EditMesh, edge: EdgeId) -> bool {
        if self.delimit.is_empty() {
            return false;
        }
        if self.delimit.contains(Delimit::SEAM) && mesh.is_edge_seam(edge) {
            return true;
        }
        if self.delimit.contains(Delimit::SHARP) && mesh.is_edge_sharp(edge) {
            return true;
        }
        if self.delimit.contains(Delimit::MATERIAL)
            && let Some((a, b)) = mesh.edge_face_pair(edge)
            && mesh.face_material(a) != mesh.face_material(b)
        {
            return true;
        }
        if self.delimit.contains(Delimit::NORMAL) && !mesh.is_edge_contiguous(edge) {
            return true;
        }
        if self.delimit.contains(Delimit::UV)
            && !self
                .uv_layers
                .iter()
                .all(|layer| mesh.is_edge_contiguous_loop_data(edge, layer))
        {
            return true;
        }
        false
    }

    fn is_vert_delimiter(&self, mesh: &EditMesh, vert: VertId) -> bool {
        if self.delimit.is_empty() {
            return false;
        }
        mesh.vert_edges(vert)
            .any(|edge| mesh.is_edge_manifold(edge) && self.is_edge_delimiter(mesh, edge))
    }
}

fn vert_edge_angle(mesh: &EditMesh, vert: VertId) -> f32 {
    match mesh.vert_edge_pair(vert) {
        Some((first, second)) => {
            let a = mesh.position(mesh.edge_other_vert(first, vert).unwrap_or(vert));
            let b = mesh.position(mesh.edge_other_vert(second, vert).unwrap_or(vert));
            std::f32::consts::PI - angle_at(a, mesh.position(vert), b)
        }
        None => RIGHT_ANGLE,
    }
}

fn edge_face_angle(mesh: &EditMesh, edge: EdgeId) -> f32 {
    match mesh.edge_face_pair(edge) {
        Some((a, b)) if mesh.is_edge_manifold(edge) => {
            angle_normalized(mesh.face_normal(a), mesh.face_normal(b))
        }
        _ => RIGHT_ANGLE,
    }
}

fn vert_cost(mesh: &EditMesh, delimiter: &Delimiter, vert: VertId) -> f32 {
    let angle = vert_edge_angle(mesh, vert);
    if let Some(edge) = mesh.vert_first_edge(vert)
        && mesh.is_edge_manifold(edge)
        && !delimiter.is_vert_delimiter(mesh, vert)
    {
        return ((angle * (1.0 / RIGHT_ANGLE))
            * (edge_face_angle(mesh, edge) * (1.0 / RIGHT_ANGLE)))
            * RIGHT_ANGLE;
    }
    angle
}

fn edge_cost(mesh: &EditMesh, delimiter: &Delimiter, edge: EdgeId) -> f32 {
    if !mesh.is_edge_manifold(edge) || delimiter.is_edge_delimiter(mesh, edge) {
        return COST_INVALID;
    }
    let Some((a, b)) = mesh.edge_face_pair(edge) else {
        return COST_INVALID;
    };
    let cos = mesh.face_normal(a).dot(mesh.face_normal(b));
    match mesh.is_edge_contiguous(edge) {
        true => -cos,
        false => cos,
    }
}

fn is_ear_collapse_degenerate(mesh: &EditMesh, ear: rikka_mesh_edit::LoopId) -> bool {
    let center = mesh.position(mesh.loop_vert(ear));
    let projection = AxisProjection::new(mesh.face_normal(mesh.loop_face(ear)));
    let project = |vert: VertId| projection.project(mesh.position(vert) - center);
    let prev = mesh.loop_prev(ear);
    let next = mesh.loop_next(ear);
    let mut tri = [
        project(mesh.loop_vert(prev)),
        Vec2::ZERO,
        project(mesh.loop_vert(next)),
    ];
    if !mesh.is_vert_edge_pair(mesh.loop_vert(prev)) {
        let adjacent = project(mesh.loop_vert(mesh.loop_prev(prev)));
        if sign(cross_tri_2d(adjacent, tri[0], tri[1]))
            != sign(cross_tri_2d(adjacent, tri[0], tri[2]))
        {
            return true;
        }
    }
    if !mesh.is_vert_edge_pair(mesh.loop_vert(next)) {
        let adjacent = project(mesh.loop_vert(mesh.loop_next(next)));
        if sign(cross_tri_2d(adjacent, tri[2], tri[1]))
            != sign(cross_tri_2d(adjacent, tri[2], tri[0]))
        {
            return true;
        }
    }
    if cross_tri_2d(tri[0], tri[1], tri[2]) < 0.0 {
        tri.swap(1, 2);
    }
    let mut lp = mesh.loop_next(next);
    loop {
        let point = project(mesh.loop_vert(lp));
        if is_point_in_tri_cw(point, tri[0], tri[1], tri[2]) {
            return true;
        }
        lp = mesh.loop_next(lp);
        if lp == prev {
            break;
        }
    }
    false
}

fn sign(value: f32) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

fn is_vert_collapse_degenerate(mesh: &EditMesh, vert: VertId) -> bool {
    let Some((first, second)) = mesh.vert_edge_pair(vert) else {
        return true;
    };
    if mesh.is_edge_wire(first) || mesh.is_edge_wire(second) {
        return false;
    }
    let a = mesh.position(mesh.edge_other_vert(first, vert).unwrap_or(vert));
    let b = mesh.position(mesh.edge_other_vert(second, vert).unwrap_or(vert));
    if cos_at(a, mesh.position(vert), b).abs() < 1.0 - f32::EPSILON {
        for lp in mesh.edge_loops(second) {
            if mesh.face_len(mesh.loop_face(lp)) > 3 {
                let pivot = match mesh.loop_vert(lp) == vert {
                    true => lp,
                    false => mesh.loop_next(lp),
                };
                if is_ear_collapse_degenerate(mesh, pivot) {
                    return true;
                }
            }
        }
    }
    false
}

pub fn dissolve_limited_geometry(
    mesh: &mut EditMesh,
    verts: &[VertId],
    edges: &[EdgeId],
    angle_limit: f32,
    use_dissolve_boundaries: bool,
    delimit: Delimit,
) -> IdSet<FaceId> {
    let delimiter = Delimiter::new(mesh, delimit);
    let angle_limit_cos_neg = -angle_limit.cos();
    let mut region: IdSet<FaceId> = IdSet::new();
    let mut verts: Vec<Option<VertId>> = verts
        .iter()
        .map(|&vert| mesh.contains_vert(vert).then_some(vert))
        .collect();
    let edges: Vec<EdgeId> = edges
        .iter()
        .copied()
        .filter(|&edge| mesh.contains_edge(edge))
        .collect();

    let wire: IdSet<EdgeId> = mesh
        .edges()
        .filter(|&edge| mesh.is_edge_wire(edge))
        .collect();
    let mut edge_index: IdMap<EdgeId, usize> = IdMap::with_capacity(mesh.edge_capacity());
    let mut heap: Heap<EdgeId> = Heap::with_capacity(edges.len());
    let mut table: Vec<Option<HeapNode>> = Vec::with_capacity(edges.len());
    for (index, &edge) in edges.iter().enumerate() {
        table.push(Some(heap.insert(edge_cost(mesh, &delimiter, edge), edge)));
        edge_index.insert(edge, index);
    }
    while let Some((top, value, edge)) = heap.top() {
        if value >= angle_limit_cos_neg {
            break;
        }
        let index = edge_index.copied(edge).unwrap_or(usize::MAX);
        let mut joined = None;
        if mesh.is_edge_manifold(edge)
            && let Some((a, b)) = mesh.edge_loop_pair(edge)
        {
            joined = mesh
                .join_face_pair_keep_shared(a, b, DoubleFace::Reuse)
                .ok()
                .map(|joined| joined.face);
        }
        match joined {
            Some(face) => {
                heap.remove(top);
                if let Some(slot) = table.get_mut(index) {
                    *slot = None;
                }
                mesh.update_face_normal(face);
                region.insert(face);
                for other in mesh.face_edges_vec(face) {
                    if let Some(other_index) = edge_index.copied(other)
                        && let Some(node) = table[other_index]
                    {
                        heap.update(node, edge_cost(mesh, &delimiter, other));
                    }
                }
            }
            None => heap.update(top, COST_INVALID),
        }
    }

    let mut vert_lookup: IdMap<VertId, usize> = IdMap::new();
    for (index, vert) in verts.iter().enumerate() {
        if let Some(vert) = vert {
            vert_lookup.insert(*vert, index);
        }
    }
    let all_edges: Vec<EdgeId> = mesh.edges().collect();
    for &edge in all_edges.iter().rev() {
        if !mesh.contains_edge(edge) || !mesh.is_edge_wire(edge) || wire.contains(edge) {
            continue;
        }
        let [v1, v2] = mesh.edge_verts(edge);
        if mesh.kill_edge(edge).is_err() {
            continue;
        }
        for vert in [v1, v2] {
            if mesh.contains_vert(vert) && mesh.vert_first_edge(vert).is_none() {
                if let Some(index) = vert_lookup.copied(vert) {
                    verts[index] = None;
                }
                mesh.kill_vert(vert).ok();
            }
        }
    }

    if use_dissolve_boundaries {
        for vert in verts.iter().flatten() {
            if mesh.contains_vert(*vert) && mesh.is_vert_edge_pair(*vert) {
                collapse_vert(mesh, *vert);
            }
        }
    } else {
        let mut vert_index: IdMap<VertId, usize> = IdMap::new();
        let mut heap: Heap<VertId> = Heap::with_capacity(verts.len());
        let mut table: Vec<Option<HeapNode>> = vec![None; verts.len()];
        for (index, vert) in verts.iter().enumerate() {
            if let Some(vert) = *vert {
                table[index] = Some(heap.insert(vert_cost(mesh, &delimiter, vert), vert));
                vert_index.insert(vert, index);
            }
        }
        while let Some((top, value, vert)) = heap.top() {
            if value >= angle_limit {
                break;
            }
            let index = vert_index.copied(vert).unwrap_or(usize::MAX);
            let mut created = None;
            if mesh.contains_vert(vert) && !is_vert_collapse_degenerate(mesh, vert) {
                created = collapse_vert(mesh, vert);
            }
            let Some(edge) = created else {
                heap.update(top, COST_INVALID);
                continue;
            };
            heap.remove(top);
            if let Some(slot) = table.get_mut(index) {
                *slot = None;
            }
            vert_index.remove(vert);
            let around: Vec<_> = mesh.edge_loops(edge).collect();
            for &lp in &around {
                mesh.update_face_normal(mesh.loop_face(lp));
            }
            for end in mesh.edge_verts(edge) {
                if let Some(end_index) = vert_index.copied(end)
                    && let Some(node) = table[end_index]
                {
                    heap.update(node, vert_cost(mesh, &delimiter, end));
                }
            }
            for &lp in &around {
                let stop = mesh.loop_prev(lp);
                let mut cycle = mesh.loop_next(mesh.loop_next(lp));
                loop {
                    let other = mesh.loop_vert(cycle);
                    if let Some(other_index) = vert_index.copied(other)
                        && let Some(node) = table[other_index]
                        && heap.value(node) == COST_INVALID
                    {
                        heap.update(node, vert_cost(mesh, &delimiter, other));
                    }
                    cycle = mesh.loop_next(cycle);
                    if cycle == stop {
                        break;
                    }
                }
            }
        }
    }

    region.retain(|face| mesh.contains_face(face));
    region
}
