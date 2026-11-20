use std::f32::consts::{PI, TAU};

use glam::Vec3;
use rikka_polymesh::{Domain, Polymesh};

use super::super::offset_shape_keys;
use super::super::vertex_group::Group;
use super::{MaterialOffsets, mark_group};
use crate::kinds::solidify::Solidify;
use crate::math::{
    angle_normalized, angle_signed_on_axis, mid_angle_weighted, normalize, shell_distance,
};

const UNUSED: u32 = u32::MAX;
const PAIRED: u32 = u32::MAX - 1;

struct Rim {
    users: Vec<u32>,
    order: Vec<usize>,
    edges: Vec<usize>,
    vertices: Vec<usize>,
    slot: Vec<u32>,
}

struct Placement {
    count: usize,
    aligned: bool,
    start: usize,
}

pub(crate) fn evaluate(solidify: &Solidify, mesh: &mut Polymesh, materials: MaterialOffsets) {
    let verts = mesh.point_count();
    let edges = mesh.edge_count();
    let faces = mesh.face_count();
    let corners = mesh.corner_count();

    let ofs_orig = -(((-solidify.offset + 1.0) * 0.5) * solidify.thickness);
    let ofs_new = solidify.thickness + ofs_orig;
    let fac_vg = solidify.thickness_vertex_group;
    let fac_vg_inv = 1.0 - fac_vg;
    let bevel_convex = solidify.bevel_convex;
    let do_flip = solidify.use_flip_normals;
    let do_clamp = solidify.thickness_clamp != 0.0;
    let do_angle_clamp = do_clamp && solidify.use_thickness_angle_clamp;
    let do_bevel_convex = bevel_convex != 0.0;
    let do_rim = solidify.use_rim;
    let do_shell = !(do_rim && solidify.use_rim_only);
    let group = Group::find(mesh, &solidify.vertex_group);
    let weighted = matches!(group, Group::Weighted(_));
    let invert = solidify.invert_vertex_group;

    let vertex_normals = mesh.point_normals();
    let face_normals = mesh.face_normals();
    let rim = rim(mesh, do_rim);
    let new_verts = match do_shell {
        true => 0,
        false => rim.vertices.len(),
    };
    let new_edges = match do_shell {
        true => 0,
        false => rim.edges.len(),
    };
    let mut non_manifold = vec![false; edges];
    let quality_normals = solidify
        .use_quality_normals
        .then(|| quality_normals(mesh, &face_normals, &vertex_normals, &mut non_manifold));

    let originals = mesh.positions.clone();
    let mut positions = originals.clone();
    match do_shell {
        true => positions.extend_from_slice(&originals),
        false => positions.extend(rim.vertices.iter().map(|&vertex| originals[vertex])),
    }
    let placement = |test: bool| -> Placement {
        if ((ofs_new >= ofs_orig) == do_flip) == test {
            Placement {
                count: verts,
                aligned: true,
                start: 0,
            }
        } else {
            Placement {
                count: if do_shell { verts } else { new_verts },
                aligned: do_shell,
                start: verts,
            }
        }
    };
    let source = |place: &Placement, slot: usize| match place.aligned {
        true => slot,
        false => rim.vertices[slot],
    };
    let mut bevel = None;

    if !solidify.use_even_offset {
        let offset = solidify.thickness.abs() * solidify.thickness_clamp;
        let offset_sq = offset * offset;
        let lengths = do_clamp.then(|| shortest_edges(mesh));
        let pairs = (do_angle_clamp || do_bevel_convex).then(|| user_pairs(mesh));
        let mut vert_angles = vec![0.5 * PI; verts];
        let mut edge_angles = vec![None; edges];
        if let Some(pairs) = &pairs {
            for (edge, pair) in pairs.iter().enumerate() {
                let Some([first, second]) = pair else {
                    continue;
                };
                let [a, b] = mesh.edges[edge];
                let axis = normalize(originals[a as usize] - originals[b as usize]);
                let angle = angle_signed_on_axis(
                    face_normals[*first as usize],
                    face_normals[*second as usize],
                    axis,
                );
                if do_angle_clamp {
                    vert_angles[a as usize] = vert_angles[a as usize].max(angle);
                    vert_angles[b as usize] = vert_angles[b as usize].max(angle);
                }
                if do_bevel_convex {
                    edge_angles[edge] = Some(angle);
                }
            }
        }
        let normal = |vertex: usize| match &quality_normals {
            Some(normals) => normals[vertex],
            None => vertex_normals[vertex],
        };
        let mut shift = |amount: f32, test: bool, new_side: bool| {
            let place = placement(test);
            for slot in 0..place.count {
                let vertex = source(&place, slot);
                let mut scaled = amount;
                if weighted {
                    let weight = group.weight(mesh, vertex, invert);
                    scaled = (fac_vg + weight * fac_vg_inv) * amount;
                }
                if let Some(lengths) = &lengths
                    && offset > f32::EPSILON
                {
                    if !weighted {
                        scaled = amount;
                    }
                    if do_angle_clamp {
                        let angle = match new_side {
                            true => TAU - vert_angles[vertex],
                            false => vert_angles[slot.min(verts - 1)],
                        };
                        let cosine = (angle * 0.5).cos();
                        if cosine > 0.0 {
                            let most = lengths[vertex].sqrt() * 0.5 / cosine;
                            if most < offset * 0.5 {
                                scaled *= most / offset * 2.0;
                            }
                        }
                    } else if lengths[vertex] < offset_sq {
                        scaled *= lengths[vertex].sqrt() / offset;
                    }
                }
                positions[place.start + slot] += normal(vertex) * scaled;
            }
        };
        if ofs_new != 0.0 {
            shift(ofs_new, false, true);
        }
        if ofs_orig != 0.0 {
            shift(ofs_orig, true, false);
        }
        if do_bevel_convex {
            bevel = Some(edge_angles);
        }
    } else {
        let check_non_manifold = solidify.use_quality_normals;
        let normals = quality_normals
            .clone()
            .unwrap_or_else(|| vertex_normals.clone());
        let mut vert_angles = vec![0.0f32; verts];
        let mut vert_accum = vec![0.0f32; verts];
        for (face, face_normal) in face_normals.iter().enumerate() {
            let range = mesh.face(face);
            let size = range.len();
            let face_verts = &mesh.corner_verts[range.clone()];
            let face_edges = &mesh.corner_edges[range];
            let mut current = size - 1;
            let mut previous_direction = normalize(
                originals[face_verts[current - 1] as usize]
                    - originals[face_verts[current] as usize],
            );
            for next in 0..size {
                let next_direction = normalize(
                    originals[face_verts[current] as usize] - originals[face_verts[next] as usize],
                );
                let angle = angle_normalized(previous_direction, next_direction).max(f32::EPSILON);
                let vertex = face_verts[current] as usize;
                vert_accum[vertex] += angle;
                if !check_non_manifold
                    || (!non_manifold[face_edges[current] as usize]
                        && !non_manifold[face_edges[next] as usize])
                {
                    vert_angles[vertex] += shell_distance(normals[vertex], *face_normal) * angle;
                } else {
                    vert_angles[vertex] += angle;
                }
                previous_direction = next_direction;
                current = next;
            }
        }
        if weighted {
            for (vertex, value) in vert_angles.iter_mut().enumerate() {
                *value *= fac_vg + group.weight(mesh, vertex, invert) * fac_vg_inv;
            }
        }
        let pairs = (do_angle_clamp || do_bevel_convex).then(|| user_pairs(mesh));
        let mut clamp_angles = vec![0.5 * PI; verts];
        let mut edge_angles = vec![None; edges];
        if let Some(pairs) = &pairs {
            for (edge, pair) in pairs.iter().enumerate() {
                let Some([first, second]) = pair else {
                    continue;
                };
                let [a, b] = mesh.edges[edge];
                let (n0, n1) = (
                    face_normals[*first as usize],
                    face_normals[*second as usize],
                );
                if do_angle_clamp {
                    let angle = PI - angle_normalized(n0, n1);
                    clamp_angles[a as usize] = clamp_angles[a as usize].max(angle);
                    clamp_angles[b as usize] = clamp_angles[b as usize].max(angle);
                }
                if do_bevel_convex {
                    let axis = normalize(originals[a as usize] - originals[b as usize]);
                    edge_angles[edge] = Some(angle_signed_on_axis(n0, n1, axis));
                }
            }
        }
        if do_clamp {
            let clamp_factor = 1.0
                + match do_angle_clamp {
                    true => solidify.offset.abs(),
                    false => 0.0,
                };
            let offset = solidify.thickness.abs() * solidify.thickness_clamp * clamp_factor;
            if offset > f32::EPSILON {
                let lengths = shortest_edges(mesh);
                let offset_sq = offset * offset;
                for vertex in 0..verts {
                    if do_angle_clamp {
                        let cosine = (clamp_angles[vertex] * 0.5).cos();
                        if cosine > 0.0 {
                            let most = lengths[vertex].sqrt() * 0.5 / cosine;
                            if most < offset * 0.5 {
                                vert_angles[vertex] *= most / offset * 2.0;
                            }
                        }
                    } else if lengths[vertex] < offset_sq {
                        vert_angles[vertex] *= lengths[vertex].sqrt() / offset;
                    }
                }
            }
        }
        if do_bevel_convex {
            bevel = Some(edge_angles);
        }
        for (amount, test) in [(ofs_new, false), (ofs_orig, true)] {
            if amount == 0.0 {
                continue;
            }
            let place = placement(test);
            for slot in 0..place.count {
                let vertex = source(&place, slot);
                if vert_accum[vertex] != 0.0 {
                    positions[place.start + slot] +=
                        normals[vertex] * (amount * (vert_angles[vertex] / vert_accum[vertex]));
                }
            }
        }
    }

    build(
        solidify,
        mesh,
        &rim,
        positions,
        Counts {
            verts,
            edges,
            faces,
            corners,
            new_edges,
            do_shell,
        },
        materials,
        bevel.as_deref(),
    );
}

#[derive(Clone, Copy)]
struct Counts {
    verts: usize,
    edges: usize,
    faces: usize,
    corners: usize,
    new_edges: usize,
    do_shell: bool,
}

#[allow(clippy::too_many_arguments)]
fn build(
    solidify: &Solidify,
    mesh: &mut Polymesh,
    rim: &Rim,
    positions: Vec<Vec3>,
    counts: Counts,
    materials: MaterialOffsets,
    bevel: Option<&[Option<f32>]>,
) {
    let Counts {
        verts,
        edges,
        faces,
        corners,
        new_edges,
        do_shell,
    } = counts;
    let stride = if do_shell { 2 } else { 1 };
    let copy_of = |vertex: u32| -> u32 {
        match do_shell {
            true => vertex + verts as u32,
            false => rim.slot[vertex as usize] + verts as u32,
        }
    };

    let original_edges = mesh.edges.clone();
    let original_corner_verts = mesh.corner_verts.clone();
    let original_corner_edges = mesh.corner_edges.clone();
    let original_offsets = mesh.face_offsets.clone();
    let original_materials: Vec<i32> = (0..faces).map(|face| mesh.material_index(face)).collect();
    let had_materials = mesh.material_indices().is_some();
    let vertex_bevel: Option<Vec<f32>> = mesh.vertex_bevel_weights().map(<[f32]>::to_vec);
    let had_edge_bevel = mesh.edge_bevel_weights().is_some();
    let had_creases = mesh.edge_creases().is_some();

    let mut point_sources: Vec<u32> = (0..verts as u32).collect();
    match do_shell {
        true => point_sources.extend(0..verts as u32),
        false => point_sources.extend(rim.vertices.iter().map(|&vertex| vertex as u32)),
    }
    let before: Vec<Vec3> = point_sources
        .iter()
        .map(|&source| mesh.positions[source as usize])
        .collect();
    mesh.gather(Domain::Point, &point_sources);
    offset_shape_keys(mesh, &before, &positions);
    mesh.positions = positions;

    let mut edge_sources: Vec<u32> = (0..edges as u32).collect();
    let mut edge_list = original_edges.clone();
    match do_shell {
        true => {
            edge_sources.extend(0..edges as u32);
            edge_list.extend(original_edges.iter().map(|edge| edge.map(copy_of)));
        }
        false => {
            edge_sources.extend(rim.edges.iter().map(|&edge| edge as u32));
            edge_list.extend(
                rim.edges
                    .iter()
                    .map(|&edge| original_edges[edge].map(copy_of)),
            );
        }
    }
    let rim_edge_start = edges * stride + new_edges;
    mesh.gather(Domain::Edge, &edge_sources);
    mesh.resize(Domain::Edge, rim_edge_start + rim.vertices.len());
    for &vertex in &rim.vertices {
        edge_list.push([vertex as u32, copy_of(vertex as u32)]);
    }
    mesh.edges = edge_list;

    let mut face_sources: Vec<u32> = (0..faces as u32).collect();
    let mut corner_sources: Vec<u32> = (0..corners as u32).collect();
    let mut corner_verts = original_corner_verts.clone();
    let mut corner_edges = original_corner_edges.clone();
    let mut offsets = original_offsets.clone();
    let mut face_materials = original_materials.clone();
    let mut reversed = vec![false; corners];
    if do_shell {
        face_sources.extend(0..faces as u32);
        for face in 0..faces {
            let start = original_offsets[face] as usize;
            let size = original_offsets[face + 1] as usize - start;
            let order: Vec<usize> = (0..size)
                .map(|slot| match slot {
                    0 => start,
                    slot => start + size - slot,
                })
                .collect();
            let mut edges_here: Vec<u32> = order
                .iter()
                .map(|&corner| original_corner_edges[corner] + edges as u32)
                .collect();
            edges_here.rotate_left(1);
            for &corner in &order {
                corner_sources.push(corner as u32);
                corner_verts.push(original_corner_verts[corner] + verts as u32);
            }
            corner_edges.extend(edges_here);
            reversed.extend(std::iter::repeat_n(true, size));
            offsets.push(corner_verts.len() as u32);
            face_materials.push(materials.shell(original_materials[face]));
        }
    }

    let mut creases: Vec<f32> = mesh
        .edge_creases()
        .map_or_else(|| vec![0.0; mesh.edge_count()], <[f32]>::to_vec);
    let mut edge_bevel: Vec<f32> = mesh
        .edge_bevel_weights()
        .map_or_else(|| vec![0.0; mesh.edge_count()], <[f32]>::to_vec);
    for (index, &vertex) in rim.vertices.iter().enumerate() {
        let edge = rim_edge_start + index;
        if let Some(weights) = &vertex_bevel {
            edge_bevel[edge] = weights[vertex];
        }
        if solidify.edge_crease_rim != 0.0 {
            creases[edge] = solidify.edge_crease_rim;
        }
    }
    let rim_slot_edge = |vertex: u32| (rim_edge_start + rim.slot[vertex as usize] as usize) as u32;
    for (index, &edge) in rim.edges.iter().enumerate() {
        let owner = rim.users[edge] as usize;
        let (face, flip) = match owner >= faces {
            true => (owner - faces, true),
            false => (owner, false),
        };
        let [a, b] = original_edges[edge];
        let start = original_offsets[face] as usize;
        let size = original_offsets[face + 1] as usize - start;
        let k1 = start + (rim.order[edge] + size - 1) % size;
        let k2 = start + rim.order[edge];
        face_sources.push(face as u32);
        corner_sources.extend([k2 as u32, k1 as u32, k1 as u32, k2 as u32]);
        reversed.extend([false; 4]);
        let inner = match do_shell {
            true => (edge + edges) as u32,
            false => (index + edges) as u32,
        };
        let (first, second) = match flip {
            false => (a, b),
            true => (b, a),
        };
        corner_verts.extend([first, second, copy_of(second), copy_of(first)]);
        corner_edges.extend([
            edge as u32,
            rim_slot_edge(second),
            inner,
            rim_slot_edge(first),
        ]);
        offsets.push(corner_verts.len() as u32);
        face_materials.push(materials.rim(original_materials[face]));
        if solidify.edge_crease_outer != 0.0 {
            creases[edge] = (creases[edge] + solidify.edge_crease_outer).min(1.0);
        }
        if solidify.edge_crease_inner != 0.0 {
            let inner = inner as usize;
            creases[inner] = (creases[inner] + solidify.edge_crease_inner).min(1.0);
        }
    }
    if let Some(angles) = bevel {
        let convex = bevel_convex_amount(solidify.bevel_convex);
        for (edge, angle) in angles.iter().enumerate() {
            let Some(angle) = *angle else {
                continue;
            };
            edge_bevel[edge] = (edge_bevel[edge]
                + match angle < PI {
                    true => convex.0,
                    false => convex.1,
                })
            .clamp(0.0, 1.0);
            if do_shell {
                let copy = edge + edges;
                edge_bevel[copy] = (edge_bevel[copy]
                    + match angle > PI {
                        true => convex.0,
                        false => convex.1,
                    })
                .clamp(0.0, 1.0);
            }
        }
    }

    mesh.gather(Domain::Face, &face_sources);
    mesh.gather(Domain::Corner, &corner_sources);
    mesh.corner_verts = corner_verts;
    mesh.corner_edges = corner_edges;
    mesh.face_offsets = offsets;
    if had_materials || face_materials.iter().any(|&material| material != 0) {
        mesh.material_indices_mut().copy_from_slice(&face_materials);
    }
    if let Some(normals) = mesh
        .attributes
        .values_mut::<Vec3>(rikka_polymesh::CUSTOM_NORMAL, Domain::Corner)
    {
        for (normal, &flipped) in normals.iter_mut().zip(&reversed) {
            if flipped {
                *normal = -*normal;
            }
        }
    }
    if had_creases
        || solidify.edge_crease_rim != 0.0
        || solidify.edge_crease_inner != 0.0
        || solidify.edge_crease_outer != 0.0
    {
        mesh.edge_creases_mut().copy_from_slice(&creases);
    }
    if had_edge_bevel || bevel.is_some() || vertex_bevel.is_some() {
        mesh.edge_bevel_weights_mut().copy_from_slice(&edge_bevel);
    }

    let rim_points = rim
        .vertices
        .iter()
        .flat_map(|&vertex| [vertex as u32, copy_of(vertex as u32)])
        .collect::<Vec<u32>>();
    mark_group(mesh, &solidify.rim_vertex_group, &rim_points);
    let shell_points: Vec<u32> = (verts as u32..mesh.point_count() as u32).collect();
    mark_group(mesh, &solidify.shell_vertex_group, &shell_points);
}

fn bevel_convex_amount(convex: f32) -> (f32, f32) {
    (convex.clamp(0.0, 1.0), convex.clamp(-1.0, 0.0))
}

fn rim(mesh: &Polymesh, enabled: bool) -> Rim {
    let edges = mesh.edge_count();
    let faces = mesh.face_count();
    let mut users = vec![UNUSED; edges];
    let mut order = vec![0usize; edges];
    let mut rim_edges = Vec::new();
    let mut vertices = Vec::new();
    let mut slot = vec![UNUSED; mesh.point_count()];
    if !enabled {
        return Rim {
            users,
            order,
            edges: rim_edges,
            vertices,
            slot,
        };
    }
    for face in 0..faces {
        let range = mesh.face(face);
        let mut previous = range.end - 1;
        for (index, corner) in range.enumerate() {
            let vertex = mesh.corner_verts[corner];
            let previous_vertex = mesh.corner_verts[previous];
            let edge = mesh.corner_edges[previous] as usize;
            if users[edge] == UNUSED {
                let [first, second] = mesh.edges[edge];
                users[edge] = match (previous_vertex > vertex) == (first < second) {
                    true => face as u32,
                    false => (face + faces) as u32,
                };
                order[edge] = index;
            } else {
                users[edge] = PAIRED;
            }
            previous = corner;
        }
    }
    let mut tagged = vec![false; mesh.point_count()];
    for (edge, &user) in users.iter().enumerate() {
        if user != UNUSED && user != PAIRED {
            let [a, b] = mesh.edges[edge];
            tagged[a as usize] = true;
            tagged[b as usize] = true;
            rim_edges.push(edge);
        }
    }
    for (vertex, &tag) in tagged.iter().enumerate() {
        if tag {
            slot[vertex] = vertices.len() as u32;
            vertices.push(vertex);
        }
    }
    Rim {
        users,
        order,
        edges: rim_edges,
        vertices,
        slot,
    }
}

fn shortest_edges(mesh: &Polymesh) -> Vec<f32> {
    let mut lengths = vec![f32::MAX; mesh.point_count()];
    for &[a, b] in &mesh.edges {
        let squared = mesh.positions[a as usize].distance_squared(mesh.positions[b as usize]);
        lengths[a as usize] = lengths[a as usize].min(squared);
        lengths[b as usize] = lengths[b as usize].min(squared);
    }
    lengths
}

fn user_pairs(mesh: &Polymesh) -> Vec<Option<[u32; 2]>> {
    let mut pairs = vec![[UNUSED; 2]; mesh.edge_count()];
    for face in 0..mesh.face_count() {
        let range = mesh.face(face);
        let mut previous = range.end - 1;
        for corner in range {
            let vertex = mesh.corner_verts[corner];
            let previous_vertex = mesh.corner_verts[previous];
            let edge = mesh.corner_edges[previous] as usize;
            let [first, second] = mesh.edges[edge];
            let side = usize::from((previous_vertex > vertex) == (first < second));
            match pairs[edge][side] {
                UNUSED => pairs[edge][side] = face as u32,
                _ => pairs[edge] = [PAIRED; 2],
            }
            previous = corner;
        }
    }
    pairs
        .into_iter()
        .map(|pair| (!pair.contains(&UNUSED) && !pair.contains(&PAIRED)).then_some(pair))
        .collect()
}

fn quality_normals(
    mesh: &Polymesh,
    face_normals: &[Vec3],
    vertex_normals: &[Vec3],
    non_manifold: &mut [bool],
) -> Vec<Vec3> {
    #[derive(Clone, Copy)]
    enum Users {
        None,
        One(u32),
        Two(u32, u32),
        Many,
    }
    let mut users = vec![Users::None; mesh.edge_count()];
    for face in 0..mesh.face_count() {
        for corner in mesh.face(face) {
            let edge = mesh.corner_edges[corner] as usize;
            users[edge] = match users[edge] {
                Users::None => Users::One(face as u32),
                Users::One(first) => Users::Two(first, face as u32),
                Users::Two(..) | Users::Many => {
                    non_manifold[edge] = true;
                    Users::Many
                }
            };
        }
    }
    let mut normals = vec![Vec3::ZERO; mesh.point_count()];
    for (edge, user) in users.iter().enumerate() {
        let normal = match *user {
            Users::Two(first, second) => {
                mid_angle_weighted(face_normals[first as usize], face_normals[second as usize])
            }
            Users::One(first) => face_normals[first as usize],
            Users::None | Users::Many => continue,
        };
        let [a, b] = mesh.edges[edge];
        normals[a as usize] += normal;
        normals[b as usize] += normal;
    }
    for (normal, fallback) in normals.iter_mut().zip(vertex_normals) {
        let normalized = normalize(*normal);
        *normal = match normalized == Vec3::ZERO {
            true => *fallback,
            false => normalized,
        };
    }
    normals
}
