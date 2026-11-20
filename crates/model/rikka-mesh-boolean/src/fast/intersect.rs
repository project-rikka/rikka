use std::collections::BTreeMap;

use glam::{Affine3A, Vec3};
use rikka_mesh_edit::{EdgeId, EditMesh, ElementKind, FaceId, IdMap, IdSet, LoopId, VertId};
use rikka_spatial::{Aabb, Bvh, Culling, Ray, TriangleBvh};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::error::Result;
use crate::fast::edgenet::{EdgeNetOptions, split_face};
use rikka_mesh_edit::math::line_point_factor;

use crate::fast::geometry::{
    LineLine, intersect_line_line, intersect_segment_triangle, point_in_triangle_prism,
    scale_triangle, triangle_normal,
};
use crate::fast::separate::split_tagged_edges;
use crate::operation::Operation;

const RAY_DEPTH_EPSILON: f32 = f32::EPSILON * 10.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct IntersectOptions {
    pub(crate) use_self: bool,
    pub(crate) separate: bool,
    pub(crate) dissolve: bool,
    pub(crate) island_connect: bool,
    pub(crate) partial_connect: bool,
    pub(crate) edge_tag: bool,
    pub(crate) boolean: Option<Operation>,
    pub(crate) epsilon: f32,
}

pub(crate) struct IntersectOutcome {
    pub(crate) changed: bool,
    pub(crate) tagged_edges: Vec<EdgeId>,
}

#[derive(Clone, Copy)]
struct Epsilon {
    eps: f32,
    eps2x_sq: f32,
    eps2x: f32,
    margin: f32,
    margin_sq: f32,
}

impl Epsilon {
    fn new(eps: f32) -> Epsilon {
        let eps2x = eps * 2.0;
        let margin = eps2x * 10.0;
        Epsilon {
            eps,
            eps2x_sq: eps2x * eps2x,
            eps2x,
            margin,
            margin_sq: margin * margin,
        }
    }
}

#[derive(Clone, Copy)]
struct LoopTri {
    verts: [VertId; 3],
    face: FaceId,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    EdgeTriEdge(usize),
    EdgeTri,
}

struct State {
    epsilon: Epsilon,
    edgetri_cache: FxHashMap<[i64; 4], VertId>,
    edge_verts: BTreeMap<EdgeId, Vec<VertId>>,
    face_edges: BTreeMap<FaceId, Vec<EdgeId>>,
    wire_edges: IdSet<EdgeId>,
    vert_dissolve: Vec<VertId>,
    tagged_verts: IdSet<VertId>,
    tagged_edges: IdSet<EdgeId>,
}

impl State {
    fn edge_verts_add(&mut self, edge: EdgeId, vert: VertId, test: bool) {
        let list = self.edge_verts.entry(edge).or_default();
        if !(test && list.contains(&vert)) {
            list.insert(0, vert);
        }
    }

    fn face_edges_add(&mut self, face: FaceId, edge: EdgeId, test: bool) {
        let list = self.face_edges.entry(face).or_default();
        if !(test && list.contains(&edge)) {
            list.insert(0, edge);
        }
    }

    fn vert_dissolve_add(&mut self, vert: VertId) {
        self.tagged_verts.insert(vert);
        self.vert_dissolve.insert(0, vert);
    }
}

fn intersect_line_tri(
    p0: Vec3,
    p1: Vec3,
    triangle: [Vec3; 3],
    normal: Vec3,
    epsilon: &Epsilon,
) -> Option<(Side, Vec3)> {
    let direction = (p0 - p1).normalize_or_zero();
    for i0 in 0..3 {
        let i1 = (i0 + 1) % 3;
        let edge_direction = (triangle[i0] - triangle[i1]).normalize_or_zero();
        if direction.dot(edge_direction).abs() >= 1.0 - epsilon.eps {
            continue;
        }
        let pair = match intersect_line_line(p0, p1, triangle[i0], triangle[i1], 0.0) {
            LineLine::None => continue,
            LineLine::Point(point) => (point, point, true),
            LineLine::Nearest(first, second) => (first, second, false),
        };
        if pair.2 || pair.0.distance_squared(pair.1) <= epsilon.margin_sq {
            let range = epsilon.margin..=1.0 - epsilon.margin;
            if range.contains(&line_point_factor(pair.1, triangle[i0], triangle[i1]))
                && range.contains(&line_point_factor(pair.0, p0, p1))
            {
                return Some((Side::EdgeTriEdge(i0), pair.0));
            }
        }
    }
    if direction.dot(normal).abs() >= epsilon.eps
        && let Some(factor) = intersect_segment_triangle(p0, p1, triangle, 0.0)
        && (epsilon.margin..=1.0 - epsilon.margin).contains(&factor)
    {
        let point = p0.lerp(p1, factor);
        let nearest = triangle
            .iter()
            .map(|corner| corner.distance_squared(point))
            .fold(f32::INFINITY, f32::min);
        if nearest >= epsilon.margin_sq {
            return Some((Side::EdgeTri, point));
        }
    }
    None
}

fn order_key(mut key: [i64; 4]) -> [i64; 4] {
    if key[2] > key[3] {
        key.swap(2, 3);
    }
    if key[0] > key[2] {
        key.swap(0, 2);
        key.swap(1, 3);
    }
    key
}

fn isect_edge_tri(
    mesh: &mut EditMesh,
    state: &mut State,
    edge: (VertId, VertId),
    triangle: &LoopTri,
    triangle_index: usize,
    normal: Vec3,
) -> Option<VertId> {
    let (mut v0, mut v1) = edge;
    if v0.index() > v1.index() {
        std::mem::swap(&mut v0, &mut v1);
    }
    let (i0, i1) = (v0.index() as i64, v1.index() as i64);
    let t = triangle.verts.map(|v| v.index() as i64);
    let keys: [[i64; 4]; 4] = [
        order_key([i0, i1, t[0], t[1]]),
        order_key([i0, i1, t[1], t[2]]),
        order_key([i0, i1, t[2], t[0]]),
        [i0, i1, triangle_index as i64, -1],
    ];
    for key in &keys {
        if let Some(&vert) = state.edgetri_cache.get(key) {
            return Some(vert);
        }
    }
    let corners = triangle.verts.map(|v| mesh.position(v));
    let (side, point) = intersect_line_tri(
        mesh.position(v0),
        mesh.position(v1),
        corners,
        normal,
        &state.epsilon,
    )?;
    let vert = mesh.add_vert(point);
    match mesh.edge_between(v0, v1) {
        Some(existing) => state.edge_verts_add(existing, vert, false),
        None => state.vert_dissolve_add(vert),
    }
    let key = match side {
        Side::EdgeTriEdge(i) => {
            if let Some(existing) =
                mesh.edge_between(triangle.verts[i], triangle.verts[(i + 1) % 3])
            {
                state.edge_verts_add(existing, vert, false);
            }
            keys[i]
        }
        Side::EdgeTri => keys[3],
    };
    state.edgetri_cache.insert(key, vert);
    Some(vert)
}

struct Visits {
    a: SmallVec<[VertId; 8]>,
    b: SmallVec<[VertId; 8]>,
}

impl Visits {
    fn push_a(&mut self, vert: VertId) {
        if !self.a.contains(&vert) {
            self.a.push(vert);
        }
    }

    fn push_b(&mut self, vert: VertId) {
        if !self.b.contains(&vert) {
            self.b.push(vert);
        }
    }
}

fn faces_share_edge(mesh: &EditMesh, a: FaceId, b: FaceId) -> bool {
    mesh.face_edges(a)
        .any(|edge| mesh.edge_faces(edge).any(|face| face == b))
}

fn vertex_edge_touch(
    mesh: &EditMesh,
    state: &mut State,
    verts: [VertId; 3],
    other: [VertId; 3],
    visited_self: &[VertId],
    visited_other: &mut SmallVec<[VertId; 8]>,
) {
    let epsilon = state.epsilon;
    for &vert in &verts {
        if visited_self.contains(&vert) {
            continue;
        }
        for e0 in 0..3 {
            let e1 = (e0 + 1) % 3;
            if visited_other.contains(&other[e0]) || visited_other.contains(&other[e1]) {
                continue;
            }
            let (start, end) = (mesh.position(other[e0]), mesh.position(other[e1]));
            let point = mesh.position(vert);
            let factor = line_point_factor(point, start, end);
            if factor > -epsilon.eps && factor < 1.0 + epsilon.eps {
                let projected = start.lerp(end, factor);
                if projected.distance_squared(point) <= epsilon.eps2x_sq {
                    if !visited_other.contains(&vert) {
                        visited_other.push(vert);
                    }
                    if let Some(edge) = mesh.edge_between(other[e0], other[e1]) {
                        state.edge_verts_add(edge, vert, true);
                    }
                    break;
                }
            }
        }
    }
}

fn isect_tri_tri(
    mesh: &mut EditMesh,
    state: &mut State,
    tris: &[LoopTri],
    a_index: usize,
    b_index: usize,
    no_shared: bool,
) {
    let (a, b) = (tris[a_index], tris[b_index]);
    let (fa, fb) = (a.verts, b.verts);
    if no_shared {
        if fa.iter().any(|v| fb.contains(v)) {
            return;
        }
    } else if faces_share_edge(mesh, a.face, b.face) {
        return;
    }
    let epsilon = state.epsilon;
    let mut visits = Visits {
        a: SmallVec::new(),
        b: SmallVec::new(),
    };
    for &va in &fa {
        for &vb in &fb {
            if mesh.position(va).distance_squared(mesh.position(vb)) <= epsilon.eps2x_sq {
                visits.push_a(va);
                visits.push_b(vb);
            }
        }
    }
    let visited_a = visits.a.clone();
    vertex_edge_touch(mesh, state, fa, fb, &visited_a, &mut visits.b);
    let visited_b = visits.b.clone();
    vertex_edge_touch(mesh, state, fb, fa, &visited_b, &mut visits.a);
    for (verts, other, own_is_a) in [(fa, fb, true), (fb, fa, false)] {
        let scaled = scale_triangle(other.map(|v| mesh.position(v)), 1.0 - epsilon.eps2x);
        for &vert in &verts {
            let visited = match own_is_a {
                true => &visits.a,
                false => &visits.b,
            };
            if visited.contains(&vert) {
                continue;
            }
            let point = mesh.position(vert);
            if let Some(projected) = point_in_triangle_prism(point, scaled)
                && projected.distance_squared(point) <= epsilon.eps2x_sq
            {
                visits.push_a(vert);
                visits.push_b(vert);
            }
        }
    }
    if visits.a.len() >= 3 && visits.b.len() >= 3 {
        return;
    }
    let normal_a = triangle_normal(fa.map(|v| mesh.position(v)));
    let normal_b = triangle_normal(fb.map(|v| mesh.position(v)));
    for (edges_of, triangle, triangle_index, normal, own_is_a) in [
        (fa, b, b_index, normal_b, true),
        (fb, a, a_index, normal_a, false),
    ] {
        for e0 in 0..3 {
            let e1 = (e0 + 1) % 3;
            let visited = match own_is_a {
                true => &visits.a,
                false => &visits.b,
            };
            if visited.contains(&edges_of[e0]) || visited.contains(&edges_of[e1]) {
                continue;
            }
            if let Some(vert) = isect_edge_tri(
                mesh,
                state,
                (edges_of[e0], edges_of[e1]),
                &triangle,
                triangle_index,
                normal,
            ) {
                visits.push_a(vert);
                visits.push_b(vert);
            }
        }
    }
    for (list, face) in [(visits.a.clone(), a.face), (visits.b.clone(), b.face)] {
        if list.len() != 2 {
            continue;
        }
        let (v0, v1) = (list[0], list[1]);
        let exists;
        let edge = match mesh.edge_between(v0, v1) {
            Some(edge) => {
                exists = true;
                state.wire_edges.insert(edge);
                if mesh.face_edges(face).any(|e| e == edge) {
                    continue;
                }
                edge
            }
            None => {
                exists = false;
                let Ok(edge) = mesh.add_edge(v0, v1) else {
                    continue;
                };
                state.wire_edges.insert(edge);
                edge
            }
        };
        state.face_edges_add(face, edge, exists);
    }
}

fn splice(mesh: &mut EditMesh, state: &mut State) -> Result<()> {
    let entries: Vec<(EdgeId, Vec<VertId>)> =
        std::mem::take(&mut state.edge_verts).into_iter().collect();
    for (edge, mut verts) in entries {
        if !mesh.contains_edge(edge) {
            continue;
        }
        let [start, _] = mesh.edge_verts(edge);
        let origin = mesh.position(start);
        if verts.len() > 1 {
            verts.sort_by(|&a, &b| {
                origin
                    .distance_squared(mesh.position(a))
                    .total_cmp(&origin.distance_squared(mesh.position(b)))
            });
        }
        let is_wire = state.wire_edges.contains(edge);
        let mut previous = start;
        for vert in verts {
            if !mesh.contains_vert(vert) || !mesh.edge_has_vert(edge, previous) {
                continue;
            }
            let other = mesh.edge_other_vert(edge, previous).unwrap_or(previous);
            let factor = line_point_factor(
                mesh.position(vert),
                mesh.position(previous),
                mesh.position(other),
            )
            .clamp(0.0, 1.0);
            let split = mesh.split_edge(edge, previous, factor)?;
            let created = split.vert;
            if mesh.edge_between(created, vert).is_none()
                && !mesh.vert_pair_shares_face(created, vert)
                && !mesh.splice_would_double_edge(created, vert)
                && !mesh.splice_would_double_face(created, vert)
            {
                mesh.splice_verts(vert, created)?;
            } else {
                let position = mesh.position(vert);
                mesh.set_position(created, position);
            }
            previous = vert;
            if is_wire {
                state.wire_edges.insert(split.edge);
            }
        }
    }
    Ok(())
}

fn other_edge(mesh: &EditMesh, vert: VertId, edge: EdgeId) -> EdgeId {
    mesh.vert_edges(vert)
        .find(|&other| other != edge)
        .unwrap_or(edge)
}

fn dissolve_knife(mesh: &mut EditMesh, state: &mut State) -> Result<()> {
    let dissolve = state.vert_dissolve.clone();
    for &vert in &dissolve {
        if state.tagged_verts.contains(vert)
            && !(mesh.contains_vert(vert) && mesh.is_vert_edge_pair(vert))
        {
            state.tagged_verts.remove(vert);
        }
    }
    let mut pairs: Vec<(VertId, VertId)> = Vec::new();
    for &vert in &dissolve {
        if !state.tagged_verts.contains(vert) {
            continue;
        }
        let Some(first) = mesh.vert_first_edge(vert) else {
            continue;
        };
        let second = mesh.disk_next(first, vert);
        if state.tagged_edges.contains(first) || state.tagged_edges.contains(second) {
            continue;
        }
        if !state.wire_edges.contains(first) || !state.wire_edges.contains(second) {
            continue;
        }
        let mut v_a = mesh.edge_other_vert(first, vert).unwrap_or(vert);
        let mut v_b = mesh.edge_other_vert(second, vert).unwrap_or(vert);
        let (a_tagged, b_tagged) = (
            state.tagged_verts.contains(v_a),
            state.tagged_verts.contains(v_b),
        );
        if a_tagged && b_tagged {
            continue;
        }
        if !a_tagged && !b_tagged {
            state.tagged_edges.insert(second);
            pairs.push((vert, v_b));
            continue;
        }
        let start_edge = match a_tagged {
            true => first,
            false => {
                std::mem::swap(&mut v_a, &mut v_b);
                second
            }
        };
        let mut step_vert = vert;
        let mut step_edge = start_edge;
        loop {
            let next = mesh
                .edge_other_vert(step_edge, step_vert)
                .unwrap_or(step_vert);
            state.tagged_edges.insert(step_edge);
            if !state.tagged_verts.contains(next) {
                pairs.push((vert, next));
                break;
            }
            step_edge = other_edge(mesh, next, step_edge);
            step_vert = next;
            state.tagged_edges.insert(step_edge);
        }
    }
    for list in state.face_edges.values_mut() {
        list.retain(|&edge| !state.tagged_edges.contains(edge));
    }
    let mut removed_faces: IdSet<FaceId> = IdSet::new();
    for edge in state.tagged_edges.to_vec() {
        if !mesh.contains_edge(edge) {
            continue;
        }
        removed_faces.extend(mesh.edge_faces(edge));
        state.wire_edges.remove(edge);
        mesh.kill_edge(edge)?;
    }
    for face in removed_faces.iter() {
        state.face_edges.remove(&face);
    }
    let mut invalid: IdSet<VertId> = IdSet::new();
    for &vert in &dissolve {
        if state.tagged_verts.contains(vert)
            && mesh.contains_vert(vert)
            && mesh.vert_first_edge(vert).is_none()
        {
            invalid.insert(vert);
            mesh.kill_vert(vert)?;
        }
    }
    for (vert, target) in pairs {
        if invalid.contains(vert)
            || invalid.contains(target)
            || !mesh.contains_vert(vert)
            || !mesh.contains_vert(target)
        {
            continue;
        }
        if mesh.edge_between(vert, target).is_none()
            && !mesh.splice_would_double_edge(vert, target)
            && !mesh.splice_would_double_face(vert, target)
        {
            mesh.splice_verts(target, vert)?;
        }
    }
    Ok(())
}

fn point_in_face(mesh: &EditMesh, face: FaceId) -> Vec3 {
    let mut triangles: Vec<[LoopId; 3]> = Vec::new();
    mesh.face_triangles(face, false, &mut triangles);
    let corners = |tri: &[LoopId; 3]| tri.map(|lp| mesh.position(mesh.loop_vert(lp)));
    let best = triangles
        .iter()
        .map(|tri| {
            let [a, b, c] = corners(tri);
            ((b - a).cross(c - a).length_squared(), *tri)
        })
        .fold(
            None,
            |best: Option<(f32, [LoopId; 3])>, (area, tri)| match best {
                Some((best_area, _)) if best_area >= area => best,
                _ => Some((area, tri)),
            },
        );
    match best {
        Some((_, tri)) => {
            let [a, b, c] = corners(&tri);
            (a + b + c) / 3.0
        }
        None => mesh.face_center_median(face),
    }
}

fn ray_hits(tree: Option<&TriangleBvh>, origin: Vec3) -> usize {
    let Some(tree) = tree else {
        return 0;
    };
    let Some(ray) = Ray::new(origin, Vec3::X) else {
        return 0;
    };
    let mut depths: Vec<f32> = tree
        .ray_cast_all(&ray, f32::MAX, Culling::None)
        .iter()
        .map(|hit| hit.distance)
        .filter(|&distance| distance >= 0.0)
        .collect();
    if depths.len() <= 1 {
        return depths.len();
    }
    depths.sort_by(f32::total_cmp);
    let mut count = 1;
    let mut last = depths[0];
    for &depth in &depths[1..] {
        if depth - last > RAY_DEPTH_EPSILON {
            last = depth;
            count += 1;
        }
    }
    count
}

fn face_groups(
    mesh: &EditMesh,
    sides: &IdMap<FaceId, i32>,
    tagged: &IdSet<EdgeId>,
) -> Vec<Vec<FaceId>> {
    let side = |face: FaceId| sides.copied(face).unwrap_or(-1);
    let mut visited: IdSet<FaceId> = IdSet::new();
    let mut groups = Vec::new();
    for start in mesh.faces().collect::<Vec<_>>() {
        if visited.contains(start) {
            continue;
        }
        visited.insert(start);
        let mut group = vec![start];
        let mut stack = vec![start];
        while let Some(face) = stack.pop() {
            for lp in mesh.face_loops_vec(face) {
                let edge = mesh.loop_edge(lp);
                if tagged.contains(edge) || mesh.loop_radial_next(lp) == lp {
                    continue;
                }
                let own = side(face);
                let consistent = mesh
                    .edge_faces(edge)
                    .all(|other| other == face || side(other) == -1 || side(other) == own);
                if !consistent {
                    continue;
                }
                for other in mesh.edge_faces(edge) {
                    if other != face && visited.insert(other) {
                        group.push(other);
                        stack.push(other);
                    }
                }
            }
        }
        groups.push(group);
    }
    groups
}

fn side_of(sides: &IdMap<FaceId, i32>, face: FaceId) -> i32 {
    sides.copied(face).unwrap_or(-1)
}

fn collapse_would_double_face(mesh: &EditMesh, vert: VertId) -> bool {
    for face in mesh.vert_faces(vert).collect::<Vec<_>>() {
        let remaining: Vec<VertId> = mesh.face_verts(face).filter(|&v| v != vert).collect();
        if remaining.len() >= 3
            && mesh
                .face_exists(&remaining)
                .is_some_and(|existing| existing != face)
        {
            return true;
        }
    }
    false
}

pub(crate) fn intersect(
    mesh: &mut EditMesh,
    sides: &mut IdMap<FaceId, i32>,
    options: &IntersectOptions,
) -> Result<IntersectOutcome> {
    let epsilon = Epsilon::new(options.epsilon);
    let tessellation = mesh.tessellate();
    let tris: Vec<LoopTri> = tessellation
        .triangles
        .iter()
        .zip(&tessellation.faces)
        .map(|(tri, &face)| LoopTri {
            verts: tri.map(|lp| mesh.loop_vert(lp)),
            face,
        })
        .collect();
    let coords: Vec<[Vec3; 3]> = tris
        .iter()
        .map(|tri| tri.verts.map(|v| mesh.position(v)))
        .collect();
    let tri_sides: Vec<i32> = tris.iter().map(|tri| side_of(sides, tri.face)).collect();
    let group_of = |want: i32| -> Vec<u32> {
        (0..tris.len() as u32)
            .filter(|&t| tri_sides[t as usize] == want)
            .collect()
    };
    let group_a = group_of(0);
    let group_b = match options.use_self {
        true => group_a.clone(),
        false => group_of(1),
    };
    let bounds = |group: &[u32]| -> Vec<Aabb> {
        group
            .iter()
            .map(|&t| Aabb::from_triangle(coords[t as usize]).expanded(epsilon.margin))
            .collect()
    };
    let tree_a = Bvh::new(&bounds(&group_a));
    let tree_b = Bvh::new(&bounds(&group_b));
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    tree_b.for_each_overlap(&tree_a, &Affine3A::IDENTITY, |i, j| {
        let (a, b) = (group_b[i as usize] as usize, group_a[j as usize] as usize);
        if !(options.use_self && a >= b) {
            pairs.push((a, b));
        }
    });
    pairs.sort_unstable();
    pairs.dedup();
    let mut state = State {
        epsilon,
        edgetri_cache: FxHashMap::default(),
        edge_verts: BTreeMap::new(),
        face_edges: BTreeMap::new(),
        wire_edges: IdSet::new(),
        vert_dissolve: Vec::new(),
        tagged_verts: IdSet::new(),
        tagged_edges: IdSet::new(),
    };
    let no_shared = options.boolean.is_some();
    for &(a, b) in &pairs {
        isect_tri_tri(mesh, &mut state, &tris, a, b, no_shared);
    }
    splice(mesh, &mut state)?;
    if options.dissolve && options.boolean.is_none() {
        dissolve_knife(mesh, &mut state)?;
    }
    let has_intersections = !state.face_edges.is_empty();
    let net_options = EdgeNetOptions {
        island_connect: options.island_connect,
        partial_connect: options.partial_connect,
    };
    let face_edges = std::mem::take(&mut state.face_edges);
    for (face, edges) in &face_edges {
        if !mesh.contains_face(*face) {
            continue;
        }
        let face_side = side_of(sides, *face);
        for created in split_face(mesh, *face, edges, net_options)? {
            sides.insert(created, face_side);
        }
    }
    let wire: Vec<EdgeId> = state
        .wire_edges
        .iter()
        .filter(|&edge| mesh.contains_edge(edge))
        .collect();
    let mut tagged: IdSet<EdgeId> = IdSet::new();
    if options.separate {
        tagged.extend(wire.iter().copied());
        mesh.clear_tags(ElementKind::Edge);
        for &edge in &wire {
            mesh.set_edge_tag(edge, true);
        }
        split_tagged_edges(mesh, &tagged)?;
        tagged = mesh
            .edges()
            .filter(|&edge| mesh.is_edge_tagged(edge))
            .collect();
        mesh.clear_tags(ElementKind::Edge);
    } else if options.boolean.is_some() || options.edge_tag {
        tagged.extend(wire.iter().copied());
    }
    let mut changed_boolean = false;
    if let Some(operation) = options.boolean {
        let side_tris = |want: i32| -> Option<TriangleBvh> {
            let list: Vec<[Vec3; 3]> = (0..tris.len())
                .filter(|&t| match options.use_self {
                    true => tri_sides[t] == 0,
                    false => tri_sides[t] == want,
                })
                .map(|t| coords[t])
                .collect();
            TriangleBvh::from_triangles(list).ok()
        };
        let trees = [side_tris(0), side_tris(1)];
        let groups = face_groups(mesh, sides, &tagged);
        let mut removed: IdSet<FaceId> = IdSet::new();
        for group in groups {
            let first = group[0];
            let own = side_of(sides, first);
            if own == -1 {
                continue;
            }
            let other = usize::from(own == 0);
            let hits = ray_hits(trees[other].as_ref(), point_in_face(mesh, first));
            let odd = hits % 2 == 1;
            let (remove, flip) = match operation {
                Operation::Intersect => (!odd, false),
                Operation::Union => (odd, false),
                Operation::Difference => (odd == (other == 1), other == 0),
            };
            if remove {
                removed.extend(group.iter().copied());
            } else if flip {
                for &face in &group {
                    mesh.reverse_face(face)?;
                }
            }
            changed_boolean |= remove || flip;
        }
        if options.dissolve {
            for &vert in &state.vert_dissolve {
                if !mesh.contains_vert(vert) || !mesh.is_vert_edge_pair(vert) {
                    continue;
                }
                let Some(edge) = mesh.vert_first_edge(vert) else {
                    continue;
                };
                if mesh.edge_faces(edge).any(|face| mesh.face_len(face) == 3)
                    || collapse_would_double_face(mesh, vert)
                {
                    continue;
                }
                mesh.collapse_vert_faces(edge, vert, 1.0, false, false)?;
            }
        }
        for face in removed.iter() {
            if mesh.contains_face(face) {
                mesh.kill_face_loose(face)?;
            }
        }
    }
    let tagged_edges: Vec<EdgeId> = tagged
        .iter()
        .filter(|&edge| mesh.contains_edge(edge))
        .collect();
    Ok(IntersectOutcome {
        changed: has_intersections || changed_boolean,
        tagged_edges,
    })
}
