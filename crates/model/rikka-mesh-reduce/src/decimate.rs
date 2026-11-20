use std::ops::ControlFlow;

use glam::{DVec3, Vec3};
use rikka_mesh_edit::{
    DoubleFace, EdgeId, EditMesh, FaceId, LocalSet, LoopId, MathLayer, NgonMethod, QuadMethod,
    SelectMode, VertId,
};
use rikka_spatial::PointTree;

use crate::common::{Counts, Removed, clear_uv_selection};
use crate::error::{Error, Result};
use crate::layers::{self, LayerValue};
use crate::math::{compare_vec3, is_quad_convex, line_point_factor, normalize};
use crate::quadric::{Quadric, plane_through};
use rikka_mesh_edit::math::{color_equal, uv_equal};
use rikka_mesh_edit::{Heap, HeapNode};

const BOUNDARY_PRESERVE_WEIGHT: f64 = 100.0;
const OPTIMIZE_EPSILON: f64 = 1.0e-8;
const TOPOLOGY_FALLBACK_EPSILON: f32 = 1.0e-12;
const COST_INVALID: f32 = f32::MAX;
const SYMMETRY_EPSILON: f32 = 0.00002;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Axis {
    X,
    #[default]
    Y,
    Z,
}

impl Axis {
    fn index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecimateOptions {
    pub ratio: f32,
    pub use_vertex_group: bool,
    pub vertex_group_factor: f32,
    pub invert_vertex_group: bool,
    pub use_symmetry: bool,
    pub symmetry_axis: Axis,
}

impl Default for DecimateOptions {
    fn default() -> DecimateOptions {
        DecimateOptions {
            ratio: 1.0,
            use_vertex_group: false,
            vertex_group_factor: 1.0,
            invert_vertex_group: false,
            use_symmetry: false,
            symmetry_axis: Axis::Y,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Collapse {
    pub factor: f32,
    pub weights: Option<Vec<f32>>,
    pub weight_factor: f32,
    pub triangulate: bool,
    pub symmetry: Option<Axis>,
    pub symmetry_epsilon: f32,
}

impl Default for Collapse {
    fn default() -> Collapse {
        Collapse {
            factor: 1.0,
            weights: None,
            weight_factor: 1.0,
            triangulate: false,
            symmetry: None,
            symmetry_epsilon: SYMMETRY_EPSILON,
        }
    }
}

pub fn decimate(mesh: &mut EditMesh, options: &DecimateOptions) -> Result<Removed> {
    if !(0.0..=1.0).contains(&options.ratio) {
        return Err(Error::OutOfRange("ratio"));
    }
    if !(options.vertex_group_factor >= 0.0 && options.vertex_group_factor <= 1000.0) {
        return Err(Error::OutOfRange("vertex_group_factor"));
    }
    let counts = Counts::new(mesh);
    if options.ratio == 1.0 {
        return Ok(counts.removed(mesh));
    }
    if !mesh.edges().any(|edge| mesh.is_edge_selected(edge)) {
        return Ok(counts.removed(mesh));
    }
    let use_group = options.use_vertex_group && !mesh.vertex_groups().is_empty();
    let group = mesh.active_vertex_group().map(|group| group as u32);
    let mut weights = vec![0.0f32; mesh.vert_capacity()];
    for vert in mesh.verts() {
        if !mesh.is_vert_selected(vert) {
            continue;
        }
        weights[vert.index()] = match use_group {
            true => {
                let weight = group.map_or(0.0, |group| mesh.vert_weight(vert, group));
                match options.invert_vertex_group {
                    true => 1.0 - weight,
                    false => weight,
                }
            }
            false => 1.0,
        };
    }
    let selected_faces = mesh
        .faces()
        .filter(|&face| mesh.is_face_selected(face))
        .count();
    let ratio = match selected_faces == mesh.face_count() || options.ratio == 0.0 {
        true => options.ratio,
        false => {
            let mut basis = 0usize;
            let mut adjacent = 0usize;
            for face in mesh.faces() {
                let len = mesh.face_len(face);
                let weight = match len > 4 {
                    true => len - 2,
                    false => 1,
                };
                basis += weight;
                if mesh
                    .face_verts(face)
                    .any(|vert| weights[vert.index()] != 0.0)
                {
                    adjacent += weight;
                }
            }
            let mut adjusted = 1.0 - options.ratio;
            adjusted *= adjacent as f32 / basis as f32;
            1.0 - adjusted
        }
    };
    decimate_collapse(
        mesh,
        &Collapse {
            factor: ratio,
            weights: Some(weights),
            weight_factor: options.vertex_group_factor,
            triangulate: false,
            symmetry: options.use_symmetry.then_some(options.symmetry_axis),
            symmetry_epsilon: SYMMETRY_EPSILON,
        },
    );
    let mut mode = mesh.select_mode();
    if !mode.intersects(SelectMode::VERT | SelectMode::EDGE) {
        mode |= SelectMode::EDGE;
    }
    mesh.select_mode_flush_with(mode, false);
    clear_uv_selection(mesh);
    mesh.update_normals();
    Ok(counts.removed(mesh))
}

struct Triangulation {
    loop_origin: Vec<i64>,
    edge_count: usize,
}

fn triangulate_begin(mesh: &mut EditMesh) -> Option<Triangulation> {
    let mut origin: Vec<(LoopId, i64)> = Vec::new();
    let mut edge_count = 0usize;
    let mut doubles: Vec<FaceId> = Vec::new();
    let faces: Vec<FaceId> = mesh.faces().collect();
    let mut cut = false;
    for face in faces {
        if mesh.face_len(face) <= 3 {
            continue;
        }
        let Ok(result) = mesh.triangulate_face(face, QuadMethod::Beauty, NgonMethod::Beauty) else {
            continue;
        };
        for &edge in &result.edges {
            for lp in mesh.edge_loops(edge) {
                origin.push((lp, face.index() as i64));
                cut = true;
            }
        }
        for &created in &result.faces {
            mesh.update_face_normal(created);
        }
        edge_count += result.edges.len();
        doubles.extend(result.doubles);
    }
    for face in doubles {
        if mesh.contains_face(face) {
            mesh.kill_face(face).ok();
        }
    }
    if !cut {
        return None;
    }
    let mut loop_origin = vec![-1i64; mesh.loop_capacity()];
    for (lp, face) in origin {
        if mesh.contains_loop(lp) {
            loop_origin[lp.index()] = face;
        }
    }
    Some(Triangulation {
        loop_origin,
        edge_count,
    })
}

fn triangulate_end(mesh: &mut EditMesh, triangulation: &Triangulation) {
    let origin = |lp: LoopId| {
        triangulation
            .loop_origin
            .get(lp.index())
            .copied()
            .unwrap_or(-1)
    };
    let can_merge = |mesh: &EditMesh, lp: LoopId, index: i64| {
        let radial = mesh.loop_radial_next(lp);
        radial != lp
            && mesh.loop_radial_next(radial) == lp
            && mesh.loop_vert(lp) != mesh.loop_vert(radial)
            && origin(lp) == index
            && origin(radial) == index
    };
    let mut merging: Vec<EdgeId> =
        Vec::with_capacity(triangulation.edge_count.min(mesh.edge_count()));
    let edges: Vec<EdgeId> = mesh.edges().collect();
    for edge in edges {
        let Some((l_a, l_b)) = mesh.edge_loop_pair(edge) else {
            continue;
        };
        let index = origin(l_a);
        if index == -1 || index != origin(l_b) || mesh.loop_vert(l_a) == mesh.loop_vert(l_b) {
            continue;
        }
        if mesh.face_len(mesh.loop_face(l_a)) == 3
            && mesh.face_len(mesh.loop_face(l_b)) == 3
            && !can_merge(mesh, mesh.loop_next(l_a), index)
            && !can_merge(mesh, mesh.loop_prev(l_a), index)
            && !can_merge(mesh, mesh.loop_next(l_b), index)
            && !can_merge(mesh, mesh.loop_prev(l_b), index)
        {
            let [v1, v2] = mesh.edge_verts(edge);
            let pick = |lp: LoopId| {
                let next = mesh.loop_vert(mesh.loop_next(lp));
                match next == v1 || next == v2 {
                    true => mesh.loop_vert(mesh.loop_prev(lp)),
                    false => next,
                }
            };
            let quad = [v1, pick(l_a), v2, pick(l_b)].map(|vert| mesh.position(vert));
            if !is_quad_convex(quad[0], quad[1], quad[2], quad[3]) {
                continue;
            }
        }
        merging.push(edge);
    }
    for edge in merging {
        if !mesh.contains_edge(edge) {
            continue;
        }
        let Some((l_a, l_b)) = mesh.edge_loop_pair(edge) else {
            continue;
        };
        let faces = [mesh.loop_face(l_a), mesh.loop_face(l_b)];
        if let Some(joined) = mesh.join_faces_keep_shared(&faces, DoubleFace::Keep).ok()
            && joined.double.is_some()
        {
            mesh.kill_face(joined.face).ok();
        }
        if mesh.contains_edge(edge) && mesh.edge_first_loop(edge).is_none() {
            mesh.kill_edge(edge).ok();
        }
    }
}

struct State<'a> {
    quadrics: Vec<Quadric>,
    weights: Option<Vec<f32>>,
    weight_factor: f32,
    heap: Heap<EdgeId>,
    table: Vec<Option<HeapNode>>,
    normals: Vec<Vec3>,
    symmetry: Option<Vec<i64>>,
    layers: &'a [MathLayer],
    vert_data: bool,
    edge_data: bool,
}

impl State<'_> {
    fn weight(&self, vert: VertId) -> Option<f32> {
        self.weights.as_ref().map(|weights| weights[vert.index()])
    }

    fn target(&self, mesh: &EditMesh, edge: EdgeId) -> DVec3 {
        let [v1, v2] = mesh.edge_verts(edge);
        let sum = Quadric::sum(&self.quadrics[v1.index()], &self.quadrics[v2.index()]);
        match sum.optimize(OPTIMIZE_EPSILON) {
            Some(point) => point,
            None => {
                let a = mesh.position(v1).as_dvec3();
                let b = mesh.position(v2).as_dvec3();
                DVec3::new(0.5 * (a.x + b.x), 0.5 * (a.y + b.y), 0.5 * (a.z + b.z))
            }
        }
    }

    fn clear(&mut self, edge: EdgeId) {
        if let Some(node) = self.table[edge.index()].take() {
            self.heap.remove(node);
        }
    }

    fn build_cost(&mut self, mesh: &EditMesh, edge: EdgeId) {
        let [v1, v2] = mesh.edge_verts(edge);
        if let (Some(w1), Some(w2)) = (self.weight(v1), self.weight(v2))
            && (w1 == 0.0 || w2 == 0.0)
        {
            self.clear(edge);
            return;
        }
        let collapsible = match mesh.edge_first_loop(edge) {
            Some(lp) if mesh.loop_radial_next(lp) == lp => mesh.face_len(mesh.loop_face(lp)) == 3,
            Some(_) => match mesh.edge_loop_pair(edge) {
                Some((a, b)) => {
                    mesh.face_len(mesh.loop_face(a)) == 3 && mesh.face_len(mesh.loop_face(b)) == 3
                }
                None => false,
            },
            None => false,
        };
        if !collapsible {
            self.clear(edge);
            return;
        }
        let target = self.target(mesh, edge);
        let mut cost = (self.quadrics[v1.index()].evaluate(target)
            + self.quadrics[v2.index()].evaluate(target)) as f32;
        cost = cost.abs();
        let p1 = mesh.position(v1);
        let p2 = mesh.position(v2);
        if cost < TOPOLOGY_FALLBACK_EPSILON {
            let alignment = self.normals[v1.index()].dot(self.normals[v2.index()]).abs();
            match (self.weight(v1), self.weight(v2)) {
                (Some(w1), Some(w2)) => {
                    let edge_weight = w1 + w2;
                    cost = alignment / (-p1.distance(p2)).min(-f32::EPSILON) - cost;
                    if edge_weight != 0.0 {
                        cost *= 1.0 + (edge_weight * self.weight_factor);
                    }
                }
                _ => {
                    cost = alignment / (-p1.distance_squared(p2)).min(-f32::EPSILON) - cost;
                }
            }
        } else if let (Some(w1), Some(w2)) = (self.weight(v1), self.weight(v2)) {
            let edge_weight = 2.0 - (w1 + w2);
            if edge_weight != 0.0 {
                cost += p1.distance(p2) * (edge_weight * self.weight_factor);
            }
        }
        let mut slot = self.table[edge.index()];
        self.heap.insert_or_update(&mut slot, cost, edge);
        self.table[edge.index()] = slot;
    }

    fn invalidate(&mut self, edge: EdgeId) {
        self.table[edge.index()] = Some(self.heap.insert(COST_INVALID, edge));
    }
}

fn build_quadrics(mesh: &EditMesh) -> Vec<Quadric> {
    let mut quadrics = vec![Quadric::default(); mesh.vert_capacity()];
    for face in mesh.faces() {
        let len = mesh.face_len(face);
        let mut center = Vec3::ZERO;
        for vert in mesh.face_verts(face) {
            center += mesh.position(vert);
        }
        center *= 1.0 / len as f32;
        let plane = plane_through(mesh.face_normal(face).as_dvec3(), center);
        let quadric = Quadric::from_plane(plane);
        for vert in mesh.face_verts(face) {
            quadrics[vert.index()].add(&quadric);
        }
    }
    for edge in mesh.edges() {
        if !mesh.is_edge_boundary(edge) {
            continue;
        }
        let [v1, v2] = mesh.edge_verts(edge);
        let Some(lp) = mesh.edge_first_loop(edge) else {
            continue;
        };
        let p1 = mesh.position(v1);
        let p2 = mesh.position(v2);
        let edge_plane = (p2 - p1).cross(mesh.face_normal(mesh.loop_face(lp)));
        let mut normal = edge_plane.as_dvec3();
        let squared = normal.x * normal.x + normal.y * normal.y + normal.z * normal.z;
        let length = match squared > 1.0e-35 {
            true => {
                let length = squared.sqrt();
                let scale = 1.0 / length;
                normal = DVec3::new(normal.x * scale, normal.y * scale, normal.z * scale);
                length
            }
            false => 0.0,
        };
        if length > f32::EPSILON as f64 {
            let center = Vec3::new(
                0.5 * (p1.x + p2.x),
                0.5 * (p1.y + p2.y),
                0.5 * (p1.z + p2.z),
            );
            let mut quadric = Quadric::from_plane(plane_through(normal, center));
            quadric.scale(BOUNDARY_PRESERVE_WEIGHT);
            quadrics[v1.index()].add(&quadric);
            quadrics[v2.index()].add(&quadric);
        }
    }
    quadrics
}

fn symmetry_map(mesh: &EditMesh, axis: usize, limit: f32) -> Vec<i64> {
    let limit_squared = limit * limit;
    let mut map = vec![-1i64; mesh.edge_capacity()];
    let edges: Vec<EdgeId> = mesh.edges().collect();
    let midpoint = |edge: EdgeId| {
        let [a, b] = mesh.edge_verts(edge);
        let (a, b) = (mesh.position(a), mesh.position(b));
        Vec3::new(0.5 * (a.x + b.x), 0.5 * (a.y + b.y), 0.5 * (a.z + b.z))
    };
    let tree = PointTree::from_indexed(
        edges
            .iter()
            .map(|&edge| (edge.index() as u32, midpoint(edge))),
    );
    for &edge in &edges {
        if map[edge.index()] != -1 {
            continue;
        }
        let mut center = midpoint(edge);
        center[axis] *= -1.0;
        let [v1, v2] = mesh.edge_verts(edge);
        let mut a = mesh.position(v1);
        let mut b = mesh.position(v2);
        a[axis] *= -1.0;
        b[axis] *= -1.0;
        let direction = b - a;
        let mut found: Option<u32> = None;
        tree.for_each_within_radius(center, limit, |nearest| {
            let other = EdgeId::new(nearest.index);
            let [o1, o2] = mesh.edge_verts(other);
            let (p1, p2) = (mesh.position(o1), mesh.position(o2));
            let mismatch = match (p2 - p1).dot(direction) > 0.0 {
                true => {
                    a.distance_squared(p1) > limit_squared || b.distance_squared(p2) > limit_squared
                }
                false => {
                    a.distance_squared(p2) > limit_squared || b.distance_squared(p1) > limit_squared
                }
            };
            if mismatch {
                return ControlFlow::Continue(());
            }
            found = Some(nearest.index);
            ControlFlow::Break(())
        });
        if let Some(other) = found {
            map[edge.index()] = other as i64;
            map[other as usize] = edge.index() as i64;
        }
    }
    map
}

fn is_flip_degenerate(mesh: &EditMesh, edge: EdgeId, target: Vec3) -> bool {
    for vert in mesh.edge_verts(edge) {
        let position = mesh.position(vert);
        for lp in mesh.vert_loops(vert) {
            let prev = mesh.loop_prev(lp);
            if mesh.loop_edge(lp) == edge || mesh.loop_edge(prev) == edge {
                continue;
            }
            let co_prev = mesh.position(mesh.loop_vert(prev));
            let co_next = mesh.position(mesh.loop_vert(mesh.loop_next(lp)));
            let other = co_prev - co_next;
            let exist = other.cross(co_prev - position);
            let optim = other.cross(co_prev - target);
            if exist.dot(optim) <= (exist.length_squared() + optim.length_squared()) * 0.01 {
                return true;
            }
        }
    }
    false
}

fn is_manifold_or_boundary(mesh: &EditMesh, edge: EdgeId) -> bool {
    match mesh.edge_first_loop(edge) {
        Some(lp) => mesh.loop_radial_next(mesh.loop_radial_next(lp)) == lp,
        None => false,
    }
}

fn is_topology_degenerate(mesh: &EditMesh, edge: EdgeId) -> bool {
    let [v1, v2] = mesh.edge_verts(edge);
    for vert in [v1, v2] {
        if mesh
            .vert_edges(vert)
            .any(|other| !is_manifold_or_boundary(mesh, other))
        {
            return true;
        }
    }
    let mut verts: LocalSet<VertId> = LocalSet::new();
    let mut faces: LocalSet<FaceId> = LocalSet::new();
    let tag = |other: EdgeId, verts: &mut LocalSet<VertId>, faces: &mut LocalSet<FaceId>| {
        verts.extend(mesh.edge_verts(other));
        if let Some(lp) = mesh.edge_first_loop(other) {
            faces.insert(mesh.loop_face(lp));
            let radial = mesh.loop_radial_next(lp);
            if radial != lp {
                faces.insert(mesh.loop_face(radial));
            }
        }
    };
    for other in mesh.vert_edges(v1) {
        tag(other, &mut verts, &mut faces);
    }
    if let Some(first) = mesh.edge_first_loop(edge) {
        for lp in [first, mesh.loop_radial_next(first)] {
            faces.remove(mesh.loop_face(lp));
            for vert in mesh.face_verts(mesh.loop_face(lp)) {
                verts.remove(vert);
            }
        }
    }
    mesh.vert_edges(v2).any(|other| {
        let [a, b] = mesh.edge_verts(other);
        verts.contains(a)
            || verts.contains(b)
            || mesh.edge_first_loop(other).is_some_and(|lp| {
                let radial = mesh.loop_radial_next(lp);
                faces.contains(mesh.loop_face(lp))
                    || (radial != lp && faces.contains(mesh.loop_face(radial)))
            })
    })
}

fn layer_equal(a: LayerValue, b: LayerValue) -> bool {
    match (a, b) {
        (LayerValue::Vec2(a), LayerValue::Vec2(b)) => uv_equal(a, b),
        (LayerValue::Color(a), LayerValue::Color(b)) => color_equal(a, b),
        _ => false,
    }
}

fn collapse_loop_data(
    mesh: &mut EditMesh,
    layers: &[MathLayer],
    lp: LoopId,
    clear: VertId,
    factor: f32,
) {
    let edge = mesh.loop_edge(lp);
    let manifold = mesh.is_edge_manifold(edge);
    let (l_clear, l_other) = match mesh.loop_vert(lp) == clear {
        true => (lp, mesh.loop_next(lp)),
        false => (mesh.loop_next(lp), lp),
    };
    let exit = manifold.then(|| mesh.loop_face(mesh.loop_radial_next(lp)));
    for side in 0..2 {
        let (first, sources, weights) = match side {
            0 => (l_clear, [l_clear, l_other], [factor, 1.0 - factor]),
            _ => (l_other, [l_other, l_clear], [1.0 - factor, factor]),
        };
        let mut previous = edge;
        let mut current = first;
        while let Some(next) = mesh.vert_step_fan_loop(current, &mut previous) {
            if next == first {
                break;
            }
            current = next;
            if exit == Some(mesh.loop_face(current)) {
                break;
            }
            for layer in layers {
                let (Some(source), Some(here)) = (
                    layers::read(mesh, layer, sources[0]),
                    layers::read(mesh, layer, current),
                ) else {
                    continue;
                };
                if layer_equal(source, here) {
                    layers::mix(mesh, layer, sources, weights, current);
                }
            }
        }
    }
}

enum Collapsed {
    Done([Option<EdgeId>; 2]),
    Refused,
}

fn collapse_edge(
    mesh: &mut EditMesh,
    state: &mut State,
    clear_edge: EdgeId,
    clear: VertId,
    factor: f32,
) -> Collapsed {
    let Some(other) = mesh.edge_other_vert(clear_edge, clear) else {
        return Collapsed::Refused;
    };
    let ordered = |mesh: &EditMesh, lp: LoopId| {
        let prev_edge = mesh.loop_edge(mesh.loop_prev(lp));
        let next_edge = mesh.loop_edge(mesh.loop_next(lp));
        match mesh.edge_has_vert(prev_edge, clear) {
            true => [prev_edge, next_edge],
            false => [next_edge, prev_edge],
        }
    };
    let (pair_a, pair_b, loops): ([EdgeId; 2], Option<[EdgeId; 2]>, Vec<LoopId>) =
        if let Some((l_a, l_b)) = mesh.edge_loop_pair(clear_edge) {
            let a = ordered(mesh, l_a);
            let b = ordered(mesh, l_b);
            if a[0] == b[0] || a[0] == b[1] || a[1] == b[0] || a[1] == b[1] {
                return Collapsed::Refused;
            }
            (a, Some(b), vec![l_a, l_b])
        } else if mesh.is_edge_boundary(clear_edge) {
            let Some(l_a) = mesh.edge_first_loop(clear_edge) else {
                return Collapsed::Refused;
            };
            (ordered(mesh, l_a), None, vec![l_a])
        } else {
            return Collapsed::Refused;
        };
    let cleared = [Some(pair_a[0]), pair_b.map(|pair| pair[0])];
    if state.vert_data {
        mesh.interpolate_vert_data_between(other, clear, other, factor);
    }
    if state.edge_data {
        mesh.interpolate_edge_data_between(pair_a[1], pair_a[0], pair_a[1], factor);
        if let Some(pair) = pair_b {
            mesh.interpolate_edge_data_between(pair[1], pair[0], pair[1], factor);
        }
    }
    if !state.layers.is_empty() {
        for &lp in &loops {
            collapse_loop_data(mesh, state.layers, lp, clear, factor);
        }
    }
    if mesh.kill_edge(clear_edge).is_err() {
        return Collapsed::Refused;
    }
    let flags = mesh.vert_flags(other).merge(mesh.vert_flags(clear));
    mesh.set_vert_flags(other, flags);
    if mesh.splice_verts(other, clear).is_err() {
        return Collapsed::Refused;
    }
    let mut survivors = [pair_a[1], pair_a[1]];
    for (index, pair) in [Some(pair_a), pair_b].into_iter().flatten().enumerate() {
        let flags = mesh.edge_flags(pair[1]).merge(mesh.edge_flags(pair[0]));
        mesh.set_edge_flags(pair[1], flags);
        mesh.splice_edges(pair[1], pair[0]).ok();
        survivors[index] = pair[1];
    }
    if let Some(map) = state.symmetry.as_mut() {
        for (index, removed) in cleared.iter().enumerate() {
            let Some(removed) = removed else {
                continue;
            };
            let mirror = map[removed.index()];
            if mirror != -1 {
                map[mirror as usize] = survivors[index].index() as i64;
            }
        }
    }
    Collapsed::Done(cleared)
}

fn decimate_edge(
    mesh: &mut EditMesh,
    state: &mut State,
    edge: EdgeId,
    target: &mut Vec3,
    check: bool,
) -> bool {
    let [other, clear] = mesh.edge_verts(edge);
    let clear_normal = state.normals[clear.index()];
    if check {
        if is_topology_degenerate(mesh, edge) {
            state.invalidate(edge);
            return false;
        }
        *target = state.target(mesh, edge).as_vec3();
        if is_flip_degenerate(mesh, edge, *target) {
            state.invalidate(edge);
            return false;
        }
    }
    let p_other = mesh.position(other);
    let p_clear = mesh.position(clear);
    let factor = match compare_vec3(p_other, p_clear, f32::EPSILON) {
        false => line_point_factor(*target, p_other, p_clear),
        true => 0.5,
    };
    let Collapsed::Done(cleared) = collapse_edge(mesh, state, edge, clear, factor) else {
        state.invalidate(edge);
        return false;
    };
    if let Some(weights) = state.weights.as_mut() {
        let weight = crate::math::interpf(weights[other.index()], weights[clear.index()], factor);
        weights[other.index()] = weight.clamp(0.0, 1.0);
    }
    mesh.set_position(other, *target);
    for removed in cleared.into_iter().flatten() {
        state.clear(removed);
    }
    let removed_quadric = state.quadrics[clear.index()];
    state.quadrics[other.index()].add(&removed_quadric);
    let normal = state.normals[other.index()];
    let s = 1.0 - factor;
    state.normals[other.index()] = normalize(Vec3::new(
        s * normal.x + factor * clear_normal.x,
        s * normal.y + factor * clear_normal.y,
        s * normal.z + factor * clear_normal.z,
    ));
    let around: Vec<EdgeId> = mesh.vert_edges(other).collect();
    for edge in around {
        state.build_cost(mesh, edge);
    }
    let corners = mesh.vert_loops(other).collect::<Vec<_>>();
    for lp in corners {
        if mesh.face_len(mesh.loop_face(lp)) == 3 {
            let outer = mesh.loop_edge(mesh.loop_next(lp));
            state.build_cost(mesh, outer);
        }
    }
    true
}

pub fn decimate_collapse(mesh: &mut EditMesh, collapse: &Collapse) {
    let triangulation = triangulate_begin(mesh);
    let layers = mesh.loop_math_layers();
    let mut weights = collapse.weights.clone();
    if let Some(weights) = weights.as_mut() {
        weights.resize(mesh.vert_capacity(), 0.0);
    }
    let mut state = State {
        quadrics: build_quadrics(mesh),
        weights,
        weight_factor: collapse.weight_factor,
        heap: Heap::with_capacity(mesh.edge_count()),
        table: vec![None; mesh.edge_capacity()],
        normals: (0..mesh.vert_capacity())
            .map(|index| {
                let vert = VertId::new(index as u32);
                match mesh.contains_vert(vert) {
                    true => mesh.vert_normal(vert),
                    false => Vec3::ZERO,
                }
            })
            .collect(),
        symmetry: None,
        layers: &layers,
        vert_data: true,
        edge_data: true,
    };
    let edges: Vec<EdgeId> = mesh.edges().collect();
    for &edge in &edges {
        state.build_cost(mesh, edge);
    }
    let target_faces = (mesh.face_count() as f32 * collapse.factor) as usize;
    if let Some(axis) = collapse.symmetry {
        state.symmetry = Some(symmetry_map(mesh, axis.index(), collapse.symmetry_epsilon));
    }
    let mut target = Vec3::ZERO;
    match collapse.symmetry {
        None => {
            while mesh.face_count() > target_faces
                && state
                    .heap
                    .top_value()
                    .is_some_and(|value| value != COST_INVALID)
            {
                let Some((_, edge)) = state.heap.pop() else {
                    break;
                };
                state.table[edge.index()] = None;
                decimate_edge(mesh, &mut state, edge, &mut target, true);
            }
        }
        Some(axis) => decimate_symmetric(mesh, &mut state, target_faces, axis.index()),
    }
    if !collapse.triangulate
        && let Some(triangulation) = &triangulation
    {
        triangulate_end(mesh, triangulation);
    }
}

fn decimate_symmetric(mesh: &mut EditMesh, state: &mut State, target_faces: usize, axis: usize) {
    while mesh.face_count() > target_faces
        && state
            .heap
            .top_value()
            .is_some_and(|value| value != COST_INVALID)
    {
        let Some((_, edge)) = state.heap.pop() else {
            break;
        };
        state.table[edge.index()] = None;
        let mirror_index = state.symmetry.as_ref().map_or(-1, |map| map[edge.index()]);
        let self_mirror = mirror_index == edge.index() as i64;
        let mut mirror: Option<EdgeId> = None;
        let mut invalidate_self = false;
        let mut invalidate_mirror = false;
        'attempt: {
            if mirror_index != -1 && !self_mirror {
                match state.table[mirror_index as usize] {
                    Some(node) => {
                        let candidate = state.heap.item(node);
                        if mesh.edge_share_vert(edge, candidate).is_some() {
                            break 'attempt;
                        }
                        mirror = Some(candidate);
                    }
                    None => {
                        invalidate_self = true;
                        break 'attempt;
                    }
                }
            }
            let ok_a = !is_topology_degenerate(mesh, edge);
            let ok_b = mirror.is_none_or(|mirror| !is_topology_degenerate(mesh, mirror));
            if !ok_a || !ok_b {
                invalidate_self = true;
                invalidate_mirror = mirror.is_some();
                break 'attempt;
            }
            let mut target = state.target(mesh, edge).as_vec3();
            if self_mirror {
                target[axis] = 0.0;
            }
            if is_flip_degenerate(mesh, edge, target) {
                invalidate_self = true;
                invalidate_mirror = mirror.is_some();
                break 'attempt;
            }
            if decimate_edge(mesh, state, edge, &mut target, false) {
                if let Some(mirror) = mirror
                    && let Some(node) = state.table[mirror_index as usize].take()
                {
                    state.heap.remove(node);
                    target[axis] *= -1.0;
                    decimate_edge(mesh, state, mirror, &mut target, false);
                }
            } else if mirror.is_some() && state.table[mirror_index as usize].is_some() {
                invalidate_mirror = true;
                break 'attempt;
            }
        }
        if invalidate_self {
            state.invalidate(edge);
        }
        if invalidate_mirror
            && let Some(mirror) = mirror
            && let Some(node) = state.table[mirror_index as usize].take()
        {
            state.heap.remove(node);
            state.invalidate(mirror);
        }
    }
}
