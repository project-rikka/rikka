use glam::{Vec2, Vec3};
use rikka_polymesh::{CUSTOM_NORMAL, Domain, Polymesh, polygon_normal};
use rikka_spatial::mean_value_weights_2d;
use rustc_hash::FxHashMap;

use crate::arrangement::Arrangement;

pub(crate) struct Built {
    pub(crate) mesh: Polymesh,
    pub(crate) face_sources: Vec<u32>,
    pub(crate) intersecting_edges: Vec<u32>,
}

pub(crate) struct Ranges<'a> {
    pub(crate) point_offsets: &'a [u32],
    pub(crate) face_offsets: &'a [u32],
}

impl Ranges<'_> {
    pub(crate) fn operand_of_face(&self, face: u32) -> u32 {
        (self.face_offsets.partition_point(|&offset| offset <= face) - 1) as u32
    }

    fn operand_of_point(&self, point: u32) -> u32 {
        (self
            .point_offsets
            .partition_point(|&offset| offset <= point)
            - 1) as u32
    }
}

fn polygon_weights(mesh: &Polymesh, face: usize, point: Vec3) -> Vec<(u32, f32)> {
    let verts = mesh.face_verts(face);
    let positions: Vec<Vec3> = verts.iter().map(|&v| mesh.positions[v as usize]).collect();
    let normal = polygon_normal(&mesh.positions, verts);
    let (tangent, bitangent) = normal.any_orthonormal_pair();
    let project = |p: Vec3| Vec2::new(p.dot(tangent), p.dot(bitangent));
    let coords: Vec<Vec2> = positions.iter().map(|&p| project(p)).collect();
    let mut weights = vec![0.0; coords.len()];
    mean_value_weights_2d(&coords, project(point), &mut weights);
    weights
        .into_iter()
        .enumerate()
        .map(|(index, weight)| (index as u32, weight))
        .collect()
}

fn matched_corners(
    mesh: &Polymesh,
    ranges: &Ranges,
    face: usize,
    origins: &[Option<u32>],
) -> Vec<Option<u32>> {
    let corners = mesh.face(face);
    let source = &mesh.corner_verts[corners.clone()];
    let len = source.len();
    let mut matched = vec![None; origins.len()];
    if origins.len() != len {
        return matched;
    }
    let operand = ranges.operand_of_face(face as u32);
    let own =
        |origin: Option<u32>| origin.filter(|&point| ranges.operand_of_point(point) == operand);
    let Some(first) = own(origins[0]) else {
        return matched;
    };
    let Some(offset) = source.iter().position(|&v| v == first) else {
        return matched;
    };
    for position in 0..len {
        let source_position = (position + offset) % len;
        let next_source = (source_position + 1) % len;
        if own(origins[position]) == Some(source[source_position])
            && own(origins[(position + 1) % len]) == Some(source[next_source])
        {
            matched[position] = Some((corners.start + source_position) as u32);
        }
    }
    matched
}

pub(crate) fn build(joined: &Polymesh, arrangement: &Arrangement, ranges: &Ranges) -> Built {
    let mut mesh = joined.clone();
    let mut vertex_face: Vec<Option<(u32, u32)>> = vec![None; arrangement.verts.len()];
    for (index, face) in arrangement.faces.iter().enumerate() {
        let key = (ranges.operand_of_face(face.face), index as u32);
        for &v in &face.verts {
            let slot = &mut vertex_face[v as usize];
            if slot.is_none_or(|best| key < best) {
                *slot = Some(key);
            }
        }
    }
    let mut point_indices: Vec<u32> = Vec::with_capacity(arrangement.verts.len());
    for (index, vertex) in arrangement.verts.iter().enumerate() {
        let source = match vertex.origin {
            Some(origin) => origin,
            None => match vertex_face[index] {
                Some((_, face)) => {
                    let source_face = arrangement.faces[face as usize].face as usize;
                    let weights = polygon_weights(joined, source_face, vertex.position);
                    let verts = joined.face_verts(source_face);
                    let sources: Vec<(u32, f32)> = weights
                        .iter()
                        .map(|&(corner, weight)| (verts[corner as usize], weight))
                        .collect();
                    mesh.push_interpolated(Domain::Point, &sources) as u32
                }
                None => mesh.push_copy(Domain::Point, 0) as u32,
            },
        };
        point_indices.push(source);
    }
    let mut corner_indices: Vec<u32> = Vec::new();
    let mut flipped_corners: Vec<bool> = Vec::new();
    let mut face_sources: Vec<u32> = Vec::with_capacity(arrangement.faces.len());
    let mut face_offsets: Vec<u32> = vec![0];
    for face in &arrangement.faces {
        let source_face = face.face as usize;
        let origins: Vec<Option<u32>> = face
            .verts
            .iter()
            .map(|&v| arrangement.verts[v as usize].origin)
            .collect();
        let matched = matched_corners(joined, ranges, source_face, &origins);
        let source_corners = joined.face(source_face);
        for (position, &v) in face.verts.iter().enumerate() {
            let corner = match matched[position] {
                Some(corner) => corner,
                None => {
                    let weights = polygon_weights(
                        joined,
                        source_face,
                        arrangement.verts[v as usize].position,
                    );
                    let sources: Vec<(u32, f32)> = weights
                        .iter()
                        .map(|&(corner, weight)| (source_corners.start as u32 + corner, weight))
                        .collect();
                    mesh.push_interpolated(Domain::Corner, &sources) as u32
                }
            };
            corner_indices.push(corner);
            flipped_corners.push(face.flipped);
        }
        face_sources.push(face.face);
        face_offsets.push(corner_indices.len() as u32);
    }
    let mut edge_lookup: FxHashMap<(u32, u32), u32> = FxHashMap::default();
    let mut edge_pairs: Vec<[u32; 2]> = Vec::new();
    let mut edge_sources: Vec<Option<u32>> = Vec::new();
    let mut corner_edges: Vec<u32> = Vec::with_capacity(corner_indices.len());
    let mut intersecting: Vec<bool> = Vec::new();
    for face in &arrangement.faces {
        let count = face.verts.len();
        for i in 0..count {
            let (a, b) = (face.verts[i], face.verts[(i + 1) % count]);
            let key = (a.min(b), a.max(b));
            let index = *edge_lookup.entry(key).or_insert_with(|| {
                edge_pairs.push([a, b]);
                edge_sources.push(None);
                intersecting.push(false);
                (edge_pairs.len() - 1) as u32
            });
            if edge_sources[index as usize].is_none() {
                edge_sources[index as usize] = face.edges[i];
            }
            intersecting[index as usize] |= face.intersect[i];
            corner_edges.push(index);
        }
    }
    let default_edge = mesh.edge_count() as u32;
    mesh.resize(Domain::Edge, default_edge as usize + 1);
    let edge_indices: Vec<u32> = edge_sources
        .iter()
        .map(|source| source.unwrap_or(default_edge))
        .collect();
    mesh.gather(Domain::Point, &point_indices);
    mesh.gather(Domain::Edge, &edge_indices);
    mesh.gather(Domain::Face, &face_sources);
    mesh.gather(Domain::Corner, &corner_indices);
    mesh.positions = arrangement
        .verts
        .iter()
        .map(|vertex| vertex.position)
        .collect();
    mesh.edges = edge_pairs;
    mesh.face_offsets = face_offsets;
    mesh.corner_verts = arrangement
        .faces
        .iter()
        .flat_map(|face| face.verts.iter().copied())
        .collect();
    mesh.corner_edges = corner_edges;
    if let Some(normals) = mesh
        .attributes
        .values_mut::<Vec3>(CUSTOM_NORMAL, Domain::Corner)
    {
        for (normal, &flipped) in normals.iter_mut().zip(&flipped_corners) {
            if flipped {
                *normal = -*normal;
            }
        }
    }
    let intersecting_edges = intersecting
        .iter()
        .enumerate()
        .filter(|(_, flag)| **flag)
        .map(|(index, _)| index as u32)
        .collect();
    Built {
        mesh,
        face_sources,
        intersecting_edges,
    }
}
