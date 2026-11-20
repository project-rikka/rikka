use std::f32::consts::{FRAC_PI_2, PI};
use std::f64::consts::PI as PI64;

use glam::Vec3;
use rikka_mesh_edit::{
    DoubleFace, EdgeId, EditMesh, FaceId, IdMap, IdSet, LoopId, MathLayer, MathLayerKind,
    SelectMode, VertId,
};

use crate::common::{clear_uv_selection, selected_faces};
use crate::error::{Result, check_range};
use crate::math::{
    angle_normalized, angle_signed_on_axis_vectors, area_tri, compare_vec3, is_quad_flip,
    normal_quad, normal_tri, normalize, rotate_normalized,
};
use rikka_mesh_edit::{Heap, HeapNode};

const MAXIMUM_IMPROVEMENT: f32 = 0.99;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JoinTrianglesOptions {
    pub face_threshold: Option<f32>,
    pub shape_threshold: Option<f32>,
    pub topology_influence: f32,
    pub uvs: bool,
    pub vcols: bool,
    pub seam: bool,
    pub sharp: bool,
    pub materials: bool,
    pub deselect_joined: bool,
}

impl Default for JoinTrianglesOptions {
    fn default() -> JoinTrianglesOptions {
        JoinTrianglesOptions {
            face_threshold: None,
            shape_threshold: None,
            topology_influence: 0.0,
            uvs: false,
            vcols: false,
            seam: false,
            sharp: false,
            materials: false,
            deselect_joined: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JoinTrianglesSettings {
    pub face_threshold: f32,
    pub shape_threshold: f32,
    pub topology_influence: f32,
    pub uvs: bool,
    pub vcols: bool,
    pub seam: bool,
    pub sharp: bool,
    pub materials: bool,
    pub deselect_joined: bool,
}

const DEFAULT_THRESHOLD: f32 = 40.0 * (PI / 180.0);

pub fn join_triangles(
    mesh: &mut EditMesh,
    options: &JoinTrianglesOptions,
) -> Result<IdSet<FaceId>> {
    for (value, name) in [
        (options.face_threshold, "face_threshold"),
        (options.shape_threshold, "shape_threshold"),
    ] {
        if let Some(value) = value {
            check_range(value, 0.0, PI, name)?;
        }
    }
    check_range(options.topology_influence, 0.0, 2.0, "topology_influence")?;
    let faces = selected_faces(mesh);
    let pair = faces.len() == 2;
    let resolve = |value: Option<f32>| match (value, pair) {
        (Some(value), _) => value,
        (None, true) => PI,
        (None, false) => DEFAULT_THRESHOLD,
    };
    let settings = JoinTrianglesSettings {
        face_threshold: resolve(options.face_threshold),
        shape_threshold: resolve(options.shape_threshold),
        topology_influence: options.topology_influence,
        uvs: options.uvs,
        vcols: options.vcols,
        seam: options.seam,
        sharp: options.sharp,
        materials: options.materials,
        deselect_joined: options.deselect_joined,
    };
    if faces.is_empty() {
        return Ok(IdSet::new());
    }
    let output = join_triangles_geometry(mesh, &faces, &settings);
    if settings.deselect_joined {
        mesh.deselect_all();
    }
    for face in output.iter() {
        mesh.select_face(face, true);
    }
    if settings.deselect_joined {
        let mode = mesh.select_mode();
        if mode.intersects(SelectMode::VERT | SelectMode::EDGE) {
            mesh.select_mode_flush_with(mode, false);
        }
    }
    clear_uv_selection(mesh);
    mesh.update_normals();
    Ok(output)
}

struct Delimit {
    seam: bool,
    sharp: bool,
    materials: bool,
    face_cos: Option<f32>,
    shape: Option<f32>,
    layers: Vec<MathLayer>,
}

impl Delimit {
    fn new(mesh: &EditMesh, settings: &JoinTrianglesSettings) -> Delimit {
        let layers = mesh
            .loop_math_layers()
            .into_iter()
            .filter(|layer| match layer.kind {
                MathLayerKind::Vec2 => settings.uvs,
                MathLayerKind::Color => settings.vcols,
            })
            .collect();
        Delimit {
            seam: settings.seam,
            sharp: settings.sharp,
            materials: settings.materials,
            face_cos: (settings.face_threshold < PI).then(|| settings.face_threshold.cos()),
            shape: (settings.shape_threshold < PI).then_some(settings.shape_threshold),
            layers,
        }
    }

    fn is_delimit(&self, mesh: &EditMesh, edge: EdgeId, first: LoopId) -> bool {
        let face_a = mesh.loop_face(first);
        let face_b = mesh.loop_face(mesh.loop_radial_next(first));
        if self.seam && mesh.is_edge_seam(edge) {
            return true;
        }
        if self.sharp && mesh.is_edge_sharp(edge) {
            return true;
        }
        if self.materials && mesh.face_material(face_a) != mesh.face_material(face_b) {
            return true;
        }
        if let Some(limit) = self.face_cos
            && mesh.face_normal(face_a).dot(mesh.face_normal(face_b)) < limit
        {
            return true;
        }
        if let Some(limit) = self.shape {
            let quad = quad_positions(mesh, first);
            if is_quad_flip(quad[0], quad[1], quad[2], quad[3]) {
                return true;
            }
            let vecs = edge_directions(&quad);
            if (0..4).any(|index| {
                (angle_normalized(vecs[index], vecs[(index + 1) % 4]) - FRAC_PI_2).abs() > limit
            }) {
                return true;
            }
        }
        self.layers
            .iter()
            .any(|layer| !mesh.is_edge_contiguous_loop_data(edge, layer))
    }
}

fn quad_verts(mesh: &EditMesh, first: LoopId) -> [VertId; 4] {
    let radial = mesh.loop_radial_next(first);
    [
        mesh.loop_vert(first),
        mesh.loop_vert(mesh.loop_prev(radial)),
        mesh.loop_vert(mesh.loop_next(first)),
        mesh.loop_vert(mesh.loop_prev(first)),
    ]
}

fn quad_positions(mesh: &EditMesh, first: LoopId) -> [Vec3; 4] {
    quad_verts(mesh, first).map(|vert| mesh.position(vert))
}

fn edge_directions(quad: &[Vec3; 4]) -> [Vec3; 4] {
    std::array::from_fn(|index| normalize(quad[index] - quad[(index + 1) % 4]))
}

fn quad_error(quad: [Vec3; 4]) -> f32 {
    let [v1, v2, v3, v4] = quad;
    let mut error = 0.0f32;
    {
        let n1 = normal_tri(v1, v2, v3);
        let n2 = normal_tri(v1, v3, v4);
        let angle_a = match compare_vec3(n1, n2, f32::EPSILON) {
            true => 0.0,
            false => angle_normalized(n1, n2),
        };
        let n1 = normal_tri(v2, v3, v4);
        let n2 = normal_tri(v4, v1, v2);
        let angle_b = match compare_vec3(n1, n2, f32::EPSILON) {
            true => 0.0,
            false => angle_normalized(n1, n2),
        };
        error += (angle_a + angle_b) / (PI * 2.0);
    }
    {
        let vecs = edge_directions(&quad);
        let diff = ((angle_normalized(vecs[0], vecs[1]) - FRAC_PI_2).abs()
            + (angle_normalized(vecs[1], vecs[2]) - FRAC_PI_2).abs()
            + (angle_normalized(vecs[2], vecs[3]) - FRAC_PI_2).abs()
            + (angle_normalized(vecs[3], vecs[0]) - FRAC_PI_2).abs())
            / (PI * 2.0);
        error += diff;
    }
    {
        let area_a = area_tri(v1, v2, v3) + area_tri(v1, v3, v4);
        let area_b = area_tri(v2, v3, v4) + area_tri(v4, v1, v2);
        let min = area_a.min(area_b);
        let max = area_a.max(area_b);
        error += match max != 0.0 {
            true => 1.0 - (min / max),
            false => 1.0,
        };
    }
    error
}

struct State {
    queue: Heap<EdgeId>,
    nodes: IdMap<EdgeId, HeapNode>,
    use_topology: bool,
    topology: f32,
}

fn rotate_to_plane(mesh: &EditMesh, quad: [VertId; 4], shared: LoopId, normal: Vec3) -> [Vec3; 4] {
    let pivot = mesh.position(mesh.loop_vert(shared));
    let next = mesh.position(mesh.loop_vert(mesh.loop_next(shared)));
    let axis = normalize(pivot - next);
    let positions = quad.map(|vert| mesh.position(vert));
    let quad_normal = normal_quad(positions[0], positions[1], positions[2], positions[3]);
    let angle = angle_signed_on_axis_vectors(normal, quad_normal, axis);
    let shared_verts = [
        mesh.loop_vert(shared),
        mesh.loop_vert(mesh.loop_next(shared)),
    ];
    std::array::from_fn(|index| match shared_verts.contains(&quad[index]) {
        true => positions[index],
        false => rotate_normalized(positions[index] - pivot, axis, angle) + pivot,
    })
}

fn alignment(
    mesh: &EditMesh,
    vecs_a: &[Vec3; 4],
    quad_b: [VertId; 4],
    shared: LoopId,
    normal: Vec3,
) -> f32 {
    let coordinates = rotate_to_plane(mesh, quad_b, shared, normal);
    let vecs_b = edge_directions(&coordinates);
    let mut error = [0.0f32; 4];
    for index in 0..4 {
        let angle_a = angle_normalized(vecs_a[index], vecs_b[index]).abs();
        let angle_b = angle_normalized(vecs_a[index], vecs_b[(index + 1) % 4]).abs();
        error[0] += angle_a;
        error[1] += angle_b;
        error[2] = (error[2] as f64 + (PI64 - angle_a as f64)) as f32;
        error[3] = (error[3] as f64 + (PI64 - angle_b as f64)) as f32;
    }
    let best = error[0].min(error[1]).min(error[2]).min(error[3]) / 4.0;
    let alignment = (1.0 - (best as f64 / (PI64 / 4.0))) as f32;
    alignment.max(0.0)
}

fn reprioritize(
    mesh: &EditMesh,
    state: &mut State,
    edge: EdgeId,
    shared: LoopId,
    vecs: &[Vec3; 4],
    error: f32,
    normal: Vec3,
) {
    let Some(node) = state.nodes.copied(edge) else {
        return;
    };
    let current = state.queue.value(node);
    if error > current {
        return;
    }
    let Some(first) = mesh.edge_first_loop(edge) else {
        return;
    };
    let quad = quad_verts(mesh, first);
    let aligned = alignment(mesh, vecs, quad, shared, normal);
    let improvement = error - current;
    let multiplier = (state.topology * aligned).min(MAXIMUM_IMPROVEMENT);
    state
        .queue
        .update(node, current + (improvement * multiplier));
}

fn neighbours(mesh: &EditMesh, inside: LoopId, items: &mut Vec<(EdgeId, LoopId)>) {
    let edge = mesh.loop_edge(inside);
    if !mesh.is_edge_manifold(edge) {
        return;
    }
    let neighbour = mesh.loop_radial_next(inside);
    if mesh.face_len(mesh.loop_face(neighbour)) != 3 {
        return;
    }
    for other in [mesh.loop_prev(neighbour), mesh.loop_next(neighbour)] {
        let other_edge = mesh.loop_edge(other);
        if mesh.is_edge_manifold(other_edge)
            && mesh.face_len(mesh.loop_face(mesh.loop_radial_next(other))) == 3
            && items.len() < 8
        {
            items.push((other_edge, neighbour));
        }
    }
}

fn reprioritize_neighbours(mesh: &EditMesh, state: &mut State, face: FaceId, error: f32) {
    let loops = mesh.face_loops_vec(face);
    if loops.len() != 4 {
        return;
    }
    let mut items: Vec<(EdgeId, LoopId)> = Vec::with_capacity(8);
    for &lp in &loops {
        neighbours(mesh, lp, &mut items);
    }
    if items.is_empty() {
        return;
    }
    let positions: Vec<Vec3> = loops
        .iter()
        .map(|&lp| mesh.position(mesh.loop_vert(lp)))
        .collect();
    let vecs: [Vec3; 4] =
        std::array::from_fn(|index| normalize(positions[index] - positions[(index + 1) % 4]));
    let normal = mesh.face_normal(face);
    for (edge, shared) in items {
        reprioritize(mesh, state, edge, shared, &vecs, error, normal);
    }
}

pub fn join_triangles_geometry(
    mesh: &mut EditMesh,
    faces: &[FaceId],
    settings: &JoinTrianglesSettings,
) -> IdSet<FaceId> {
    let delimit = Delimit::new(mesh, settings);
    let faces: Vec<FaceId> = faces
        .iter()
        .copied()
        .filter(|&face| mesh.contains_face(face))
        .collect();
    let mut input: IdSet<FaceId> = IdSet::new();
    let mut output: IdSet<FaceId> = IdSet::new();
    for &face in &faces {
        if mesh.face_len(face) == 3 {
            input.insert(face);
            if settings.deselect_joined {
                output.insert(face);
            }
        }
    }
    let mut state = State {
        queue: Heap::with_capacity(1),
        nodes: IdMap::new(),
        use_topology: settings.topology_influence != 0.0,
        topology: settings.topology_influence,
    };
    let edges: Vec<EdgeId> = mesh.edges().collect();
    for edge in edges {
        let Some((first, second)) = mesh.edge_loop_pair(edge) else {
            continue;
        };
        if !input.contains(mesh.loop_face(first))
            || !input.contains(mesh.loop_face(second))
            || delimit.is_delimit(mesh, edge, first)
        {
            continue;
        }
        let error = quad_error(quad_positions(mesh, first));
        let node = state.queue.insert(error, edge);
        if state.use_topology {
            state.nodes.insert(edge, node);
        }
    }
    if state.use_topology && !state.queue.is_empty() {
        for &face in &faces {
            if mesh.face_len(face) != 4 {
                continue;
            }
            let positions: Vec<Vec3> = mesh
                .face_verts(face)
                .map(|vert| mesh.position(vert))
                .collect();
            let error = quad_error([positions[0], positions[1], positions[2], positions[3]])
                * (2.0 - (state.topology * MAXIMUM_IMPROVEMENT));
            reprioritize_neighbours(mesh, &mut state, face, error);
        }
    }
    while let Some((error, edge)) = state.queue.pop() {
        state.nodes.remove(edge);
        let Some(joined) = join_by_edge(mesh, edge) else {
            continue;
        };
        input.remove(joined);
        output.remove(joined);
        if !settings.deselect_joined {
            output.insert(joined);
        }
        if state.use_topology {
            reprioritize_neighbours(mesh, &mut state, joined, error);
        }
    }
    output.retain(|face| mesh.contains_face(face));
    output
}

fn join_by_edge(mesh: &mut EditMesh, edge: EdgeId) -> Option<FaceId> {
    if !mesh.contains_edge(edge) {
        return None;
    }
    let (first, second) = mesh.edge_loop_pair(edge)?;
    if mesh.face_len(mesh.loop_face(first)) != 3 || mesh.face_len(mesh.loop_face(second)) != 3 {
        return None;
    }
    let quad = [
        mesh.loop_vert(first),
        mesh.loop_vert(mesh.loop_prev(second)),
        mesh.loop_vert(mesh.loop_next(first)),
        mesh.loop_vert(mesh.loop_prev(first)),
    ];
    if mesh.face_exists(&quad).is_some() {
        return None;
    }
    mesh.join_face_pair(first, second, DoubleFace::Reuse)
        .ok()
        .map(|joined| joined.face)
}
